mod dummy;
use dummy::DummyDecision;

use crate::{block::ChainType, peers::PeerMessenger};
use copycat_utils::{CopycatError, NodeId};

use async_trait::async_trait;

use tokio::sync::mpsc;

use std::sync::Arc;

#[async_trait]
pub trait Decision<BlockType>: Sync + Send {
    async fn decide(&self, block: &BlockType) -> Result<bool, CopycatError>;
}

fn get_decision<TxnType, BlockType>(
    chain_type: ChainType,
    peer_messenger: Arc<PeerMessenger<TxnType, BlockType>>,
    peer_consensus_recv: mpsc::UnboundedReceiver<(NodeId, Arc<Vec<u8>>)>,
) -> Box<dyn Decision<BlockType>> {
    match chain_type {
        ChainType::Dummy => Box::new(DummyDecision::new()),
        ChainType::Bitcoin => todo!(),
    }
}

pub async fn decision_thread<TxnType, BlockType>(
    id: NodeId,
    chain_type: ChainType,
    peer_messenger: Arc<PeerMessenger<TxnType, BlockType>>,
    peer_consensus_recv: mpsc::UnboundedReceiver<(NodeId, Arc<Vec<u8>>)>,
    mut block_ready_recv: mpsc::Receiver<Arc<BlockType>>,
    commit_send: mpsc::Sender<Arc<BlockType>>,
) {
    let decision_stage = get_decision(chain_type, peer_messenger, peer_consensus_recv);

    loop {
        let block = match block_ready_recv.recv().await {
            Some(blk) => blk,
            None => {
                log::error!("Node {id}: block_ready pipe closed unexpectedly");
                continue;
            }
        };

        match decision_stage.decide(&block).await {
            Ok(decision) => {
                if !decision {
                    log::trace!("Node {id}: block should not commit, ignoring...");
                    continue;
                }

                if let Err(e) = commit_send.send(block).await {
                    log::error!("Node {id}: failed to send to commit pipe: {e:?}");
                    continue;
                }
            }
            Err(e) => {
                log::error!("Node {id}: failed to make decision: {e:?}");
                continue;
            }
        }
    }
}
