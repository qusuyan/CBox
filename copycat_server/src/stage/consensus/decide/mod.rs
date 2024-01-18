mod dummy;
use dummy::DummyDecision;
mod bitcoin;
use bitcoin::BitcoinDecision;

use crate::peers::PeerMessenger;
use copycat_protocol::block::Block;
use copycat_protocol::ChainType;
use copycat_utils::{CopycatError, NodeId};

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::mpsc;

#[async_trait]
pub trait Decision: Sync + Send {
    async fn new_tail(&mut self, new_tail: Vec<Arc<Block>>) -> Result<(), CopycatError>;
    async fn commit_ready(&self) -> Result<(), CopycatError>;
    async fn next_to_commit(&mut self) -> Result<Arc<Block>, CopycatError>;
}

fn get_decision(
    chain_type: ChainType,
    peer_messenger: Arc<PeerMessenger>,
    peer_consensus_recv: mpsc::Receiver<(NodeId, Arc<Vec<u8>>)>,
) -> Box<dyn Decision> {
    match chain_type {
        ChainType::Dummy => Box::new(DummyDecision::new()),
        ChainType::Bitcoin => Box::new(BitcoinDecision::new()),
    }
}

pub async fn decision_thread(
    chain_type: ChainType,
    peer_messenger: Arc<PeerMessenger>,
    peer_consensus_recv: mpsc::Receiver<(NodeId, Arc<Vec<u8>>)>,
    mut block_ready_recv: mpsc::Receiver<Vec<Arc<Block>>>,
    commit_send: mpsc::Sender<Arc<Block>>,
) {
    log::info!("decision stage starting...");

    let mut decision_stage = get_decision(chain_type, peer_messenger, peer_consensus_recv);

    loop {
        tokio::select! {
            new_tail = block_ready_recv.recv() => {
                log::debug!("got new chain tail: {new_tail:?}");
                let new_tail = match new_tail {
                    Some(tail) => tail,
                    None => {
                        log::error!("block_ready pipe closed unexpectedly");
                        return;
                    }
                };

                if let Err(e) = decision_stage.new_tail(new_tail).await {
                    log::error!("failed to record new chain tail: {e:?}");
                    continue;
                }
            },
            commit_ready = decision_stage.commit_ready() => {
                if let Err(e) = commit_ready {
                    log::error!("waiting for commit ready block failed: {e:?}");
                    continue;
                }

                let block_to_commit = match decision_stage.next_to_commit().await {
                    Ok(blk) => blk,
                    Err(e) => {
                        log::error!("getting commit ready block failed: {e:?}");
                        continue;
                    }
                };

                log::debug!("committing new block {block_to_commit:?}");

                if let Err(e) = commit_send.send(block_to_commit).await {
                                    log::error!("failed to send to commit pipe: {e:?}");
                                    continue;
                                }

            },
        }
    }
}
