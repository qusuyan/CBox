mod dummy;
use dummy::DummyDecision;
mod bitcoin;
use bitcoin::BitcoinDecision;

use crate::protocol::block::Block;
use crate::utils::{CopycatError, NodeId};
use crate::{config::Config, peers::PeerMessenger};

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::mpsc;

#[async_trait]
pub trait Decision: Sync + Send {
    async fn new_tail(&mut self, new_tail: Vec<Arc<Block>>) -> Result<(), CopycatError>;
    async fn commit_ready(&self) -> Result<(), CopycatError>;
    async fn next_to_commit(&mut self) -> Result<(u64, Arc<Block>), CopycatError>;
}

fn get_decision(
    id: NodeId,
    config: Config,
    peer_messenger: Arc<PeerMessenger>,
    peer_consensus_recv: mpsc::Receiver<(NodeId, Arc<Vec<u8>>)>,
) -> Box<dyn Decision> {
    match config {
        Config::Dummy => Box::new(DummyDecision::new()),
        Config::Bitcoin { config } => Box::new(BitcoinDecision::new(id, config)),
    }
}

pub async fn decision_thread(
    id: NodeId,
    config: Config,
    peer_messenger: Arc<PeerMessenger>,
    peer_consensus_recv: mpsc::Receiver<(NodeId, Arc<Vec<u8>>)>,
    mut block_ready_recv: mpsc::Receiver<Vec<Arc<Block>>>,
    commit_send: mpsc::Sender<(u64, Arc<Block>)>,
) {
    pf_info!(id; "decision stage starting...");

    let mut decision_stage = get_decision(id, config, peer_messenger, peer_consensus_recv);

    loop {
        tokio::select! {
            new_tail = block_ready_recv.recv() => {
                pf_debug!(id; "got new chain tail: {:?}", new_tail);
                let new_tail = match new_tail {
                    Some(tail) => tail,
                    None => {
                        pf_error!(id; "block_ready pipe closed unexpectedly");
                        return;
                    }
                };

                if let Err(e) = decision_stage.new_tail(new_tail).await {
                    pf_error!(id; "failed to record new chain tail: {:?}", e);
                    continue;
                }
            },
            commit_ready = decision_stage.commit_ready() => {
                if let Err(e) = commit_ready {
                    pf_error!(id; "waiting for commit ready block failed: {:?}", e);
                    continue;
                }

                let (height, block_to_commit) = match decision_stage.next_to_commit().await {
                    Ok(blk) => blk,
                    Err(e) => {
                        pf_error!(id; "getting commit ready block failed: {:?}", e);
                        continue;
                    }
                };

                pf_debug!(id; "committing new block {:?}, height {}", block_to_commit, height);

                if let Err(e) = commit_send.send((height, block_to_commit)).await {
                    pf_error!(id; "failed to send to commit pipe: {:?}", e);
                    continue;
                }

            },
        }
    }
}
