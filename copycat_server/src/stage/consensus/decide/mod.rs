mod dummy;
use dummy::DummyDecision;

use crate::peers::PeerMessenger;
use crate::state::ChainState;
use copycat_protocol::block::Block;
use copycat_protocol::ChainType;
use copycat_utils::{CopycatError, NodeId};

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::mpsc;

#[async_trait]
pub trait Decision: Sync + Send {
    async fn decide(&self, block: &Block) -> Result<bool, CopycatError>;
}

fn get_decision(
    chain_type: ChainType,
    peer_messenger: Arc<PeerMessenger>,
    state: Arc<ChainState>,
    peer_consensus_recv: mpsc::UnboundedReceiver<(NodeId, Arc<Vec<u8>>)>,
) -> Box<dyn Decision> {
    match chain_type {
        ChainType::Dummy => Box::new(DummyDecision::new()),
        ChainType::Bitcoin => todo!(),
    }
}

pub async fn decision_thread(
    id: NodeId,
    chain_type: ChainType,
    state: Arc<ChainState>,
    peer_messenger: Arc<PeerMessenger>,
    peer_consensus_recv: mpsc::UnboundedReceiver<(NodeId, Arc<Vec<u8>>)>,
    mut block_ready_recv: mpsc::Receiver<Arc<Block>>,
    commit_send: mpsc::Sender<Arc<Block>>,
) {
    log::trace!("Node {id}: Decision stage starting...");

    let decision_stage = get_decision(chain_type, peer_messenger, state, peer_consensus_recv);

    loop {
        let block = match block_ready_recv.recv().await {
            Some(blk) => blk,
            None => {
                log::error!("Node {id}: block_ready pipe closed unexpectedly");
                continue;
            }
        };

        log::trace!("Node {id}: got new block {block:?}");

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
