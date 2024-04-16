mod dummy;
use dummy::DummyDecision;
mod bitcoin;
use bitcoin::BitcoinDecision;
mod avalanche;
use avalanche::AvalancheDecision;

use crate::context::BlkCtx;
use crate::protocol::block::Block;
use crate::transaction::Txn;
use crate::utils::{CopycatError, NodeId};
use crate::CryptoScheme;
use crate::{config::Config, peers::PeerMessenger};

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{Duration, Instant};

#[async_trait]
pub trait Decision: Sync + Send {
    async fn new_tail(
        &mut self,
        src: NodeId,
        new_tail: Vec<(Arc<Block>, Arc<BlkCtx>)>,
    ) -> Result<(), CopycatError>;
    async fn commit_ready(&self) -> Result<(), CopycatError>;
    async fn next_to_commit(&mut self) -> Result<(u64, Vec<Arc<Txn>>), CopycatError>;
    async fn handle_peer_msg(&mut self, src: NodeId, content: Vec<u8>) -> Result<(), CopycatError>;
}

fn get_decision(
    id: NodeId,
    crypto_scheme: CryptoScheme,
    config: Config,
    peer_messenger: Arc<PeerMessenger>,
    pmaker_feedback_send: mpsc::Sender<Vec<u8>>,
) -> Box<dyn Decision> {
    match config {
        Config::Dummy => Box::new(DummyDecision::new()),
        Config::Bitcoin { config } => Box::new(BitcoinDecision::new(id, config)),
        Config::Avalanche { config } => Box::new(AvalancheDecision::new(
            id,
            crypto_scheme,
            config,
            peer_messenger,
            pmaker_feedback_send,
        )),
    }
}

pub async fn decision_thread(
    id: NodeId,
    crypto_scheme: CryptoScheme,
    config: Config,
    peer_messenger: Arc<PeerMessenger>,
    mut peer_consensus_recv: mpsc::Receiver<(NodeId, Vec<u8>)>,
    mut block_ready_recv: mpsc::Receiver<(NodeId, Vec<(Arc<Block>, Arc<BlkCtx>)>)>,
    commit_send: mpsc::Sender<(u64, Vec<Arc<Txn>>)>,
    pmaker_feedback_send: mpsc::Sender<Vec<u8>>,
) {
    pf_info!(id; "decision stage starting...");

    let mut decision_stage = get_decision(
        id,
        crypto_scheme,
        config,
        peer_messenger,
        pmaker_feedback_send,
    );

    let mut report_timeout = Instant::now() + Duration::from_secs(60);
    let mut blks_sent = 0;
    let mut txns_sent = 0;
    let mut blks_recv = 0;
    let mut txns_recv = 0;

    loop {
        tokio::select! {
            new_tail = block_ready_recv.recv() => {
                let (src, new_tail) = match new_tail {
                    Some(tail) => tail,
                    None => {
                        pf_error!(id; "block_ready pipe closed unexpectedly");
                        return;
                    }
                };

                pf_debug!(id; "got new chain tail from {}: {:?}", src, new_tail);
                for (blk, _) in new_tail.iter() {
                    blks_recv += 1;
                    txns_recv += blk.txns.len();
                }

                if let Err(e) = decision_stage.new_tail(src, new_tail).await {
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

                pf_debug!(id; "committing new block of length {:?}, height {}", block_to_commit.len(), height);

                blks_sent += 1;
                txns_sent += block_to_commit.len();

                if let Err(e) = commit_send.send((height, block_to_commit)).await {
                    pf_error!(id; "failed to send to commit pipe: {:?}", e);
                    continue;
                }

            },
            peer_msg = peer_consensus_recv.recv() => {
                let (src, msg) =  match peer_msg {
                    Some(msg) => msg,
                    None => {
                        pf_error!(id; "peer consensus recv pipe closed unexpectedly");
                        continue;
                    }
                };

                if let Err(e) = decision_stage.handle_peer_msg(src, msg).await {
                    pf_error!(id; "failed to handle message from peer {}: {:?}", src, e);
                    continue;
                }
            },
            _ = tokio::time::sleep_until(report_timeout) => {
                pf_info!(id; "In the last minute: blks_recv: {}, txns_recv: {}, blks_sent: {}, txns_sent: {}", blks_recv, txns_recv, blks_sent, txns_sent);
                blks_recv = 0;
                txns_recv = 0;
                blks_sent = 0;
                txns_sent = 0;
                report_timeout = Instant::now() + Duration::from_secs(60);
            }
        }
    }
}
