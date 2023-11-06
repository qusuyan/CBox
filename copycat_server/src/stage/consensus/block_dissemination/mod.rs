use crate::peers::PeerMessenger;
use copycat_utils::{CopycatError, NodeId};

use async_trait::async_trait;

use serde::{de::DeserializeOwned, Serialize};
use tokio::sync::mpsc;

use std::sync::Arc;

pub enum BlockDisseminationType {
    Broadcast,
}

#[async_trait]
pub trait BlockDissemination<BlockType>: Sync + Send {
    async fn disseminate(&self, block: &BlockType) -> Result<(), CopycatError>;
}

fn get_block_dissemination<TxnType, BlockType>(
    block_dissemination_type: BlockDisseminationType,
    peer_messenger: Arc<PeerMessenger<TxnType, BlockType>>,
) -> Box<dyn BlockDissemination<BlockType>> {
    todo!();
}

pub async fn block_dissemination_thread<TxnType, BlockType>(
    id: NodeId,
    block_dissemination_type: BlockDisseminationType,
    peer_messenger: Arc<PeerMessenger<TxnType, BlockType>>,
    mut new_block_recv: mpsc::Receiver<Arc<BlockType>>,
    block_ready_send: mpsc::Sender<Arc<BlockType>>,
) where
    TxnType: 'static + std::fmt::Debug + Serialize + DeserializeOwned + Sync + Send,
    BlockType: 'static + Clone + std::fmt::Debug + Serialize + DeserializeOwned + Sync + Send,
{
    let block_dissemination_stage =
        get_block_dissemination(block_dissemination_type, peer_messenger);

    loop {
        let new_blk = match new_block_recv.recv().await {
            Some(blk) => blk,
            None => {
                log::error!("Node {id}: new_block pipe closed unexpectedly");
                continue;
            }
        };

        if let Err(e) = block_dissemination_stage.disseminate(&new_blk).await {
            log::error!("Node {id}: failed to disseminate new block: {e:?}");
            continue;
        }

        if let Err(e) = block_ready_send.send(new_blk).await {
            log::error!("Node {id}: failed to send to block_ready pipe: {e:?}");
            continue;
        }
    }
}
