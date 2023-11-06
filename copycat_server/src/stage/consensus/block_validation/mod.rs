use async_trait::async_trait;

use copycat_utils::{CopycatError, NodeId};

use std::sync::Arc;
use tokio::sync::mpsc;

#[async_trait]
pub trait BlockValidation<BlockType>: Sync + Send {
    async fn validate(&self, block: &BlockType) -> Result<bool, CopycatError>;
}

pub enum BlockValidationType {
    Dummy,
}

fn get_block_validation<BlockType>(
    block_validation_type: BlockValidationType,
) -> Box<dyn BlockValidation<BlockType>> {
    todo!();
}

pub async fn block_validation_thread<BlockType>(
    id: NodeId,
    block_validation_type: BlockValidationType,
    mut peer_blk_recv: mpsc::UnboundedReceiver<(NodeId, Arc<BlockType>)>,
    block_ready_send: mpsc::Sender<Arc<BlockType>>,
) {
    let block_validation_stage = get_block_validation(block_validation_type);

    loop {
        let (src, new_block) = match peer_blk_recv.recv().await {
            Some(blk) => blk,
            None => {
                log::error!("Node {id}: peer_blk pipe closed unexpectedly");
                continue;
            }
        };

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
