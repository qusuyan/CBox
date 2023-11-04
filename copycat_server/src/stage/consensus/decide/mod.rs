use crate::peers::PeerMessenger;
use copycat_utils::{CopycatError, NodeId};

use async_trait::async_trait;

use tokio::sync::mpsc;

#[async_trait]
pub trait Decision<BlockType> {
    async fn decide(&self, block: &BlockType) -> Result<bool, CopycatError>;
}

pub enum DecisionType {}

fn get_decision<TxnType, BlockType>(
    decision_type: DecisionType,
    peer_messenger: PeerMessenger<TxnType, BlockType>,
) -> Box<dyn Decision<BlockType>> {
    todo!();
}

pub async fn decision_thread<TxnType, BlockType>(
    id: NodeId,
    decision_type: DecisionType,
    peer_messenger: PeerMessenger<TxnType, BlockType>,
    mut block_ready_recv: mpsc::Receiver<BlockType>,
    commit_send: mpsc::Sender<BlockType>,
) {
    let decision_stage = get_decision(decision_type, peer_messenger);

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
