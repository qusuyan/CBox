mod dummy;
use dummy::DummyBlockValidation;

use crate::state::ChainState;
use copycat_protocol::block::Block;
use copycat_protocol::ChainType;
use copycat_utils::{CopycatError, NodeId};

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::mpsc;

#[async_trait]
pub trait BlockValidation: Sync + Send {
    async fn validate(&self, block: &Block) -> Result<bool, CopycatError>;
}

fn get_block_validation(chain_type: ChainType, state: Arc<ChainState>) -> Box<dyn BlockValidation> {
    match chain_type {
        ChainType::Dummy => Box::new(DummyBlockValidation::new()),
        ChainType::Bitcoin => todo!(),
    }
}

pub async fn block_validation_thread(
    id: NodeId,
    chain_type: ChainType,
    state: Arc<ChainState>,
    mut peer_blk_recv: mpsc::UnboundedReceiver<(NodeId, Arc<Block>)>,
    block_ready_send: mpsc::Sender<Arc<Block>>,
) {
    log::trace!("Node {id}: Txn Validation stage starting...");

    let block_validation_stage = get_block_validation(chain_type, state);

    loop {
        let (src, new_block) = match peer_blk_recv.recv().await {
            Some((src, blk)) => {
                if src == id {
                    // ignore blocks proposed by myself
                    continue;
                }
                (src, blk)
            }
            None => {
                log::error!("Node {id}: peer_blk pipe closed unexpectedly");
                continue;
            }
        };

        log::trace!("Node {id}: got from {src} new block {new_block:?}");

        match block_validation_stage.validate(&new_block).await {
            Ok(valid) => {
                if !valid {
                    log::warn!("Node {id}: got invalid block from {src}, ignoring...");
                    continue;
                }

                if let Err(e) = block_ready_send.send(new_block).await {
                    log::error!("Node {id}: failed to send to block_ready pipe: {e:?}");
                    continue;
                }
            }
            Err(e) => {
                log::error!("Node {id}: failed to validate block: {e:?}");
                continue;
            }
        }
    }
}
