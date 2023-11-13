mod broadcast;
use broadcast::BroadcastBlockDissemination;

use crate::peers::PeerMessenger;
use copycat_protocol::block::Block;
use copycat_protocol::DissemPattern;
use copycat_utils::{CopycatError, NodeId};

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::mpsc;

#[async_trait]
pub trait BlockDissemination: Sync + Send {
    async fn disseminate(&self, block: &Block) -> Result<(), CopycatError>;
}

fn get_block_dissemination(
    dissem_pattern: DissemPattern,
    peer_messenger: Arc<PeerMessenger>,
) -> Box<dyn BlockDissemination> {
    match dissem_pattern {
        DissemPattern::Broadcast => Box::new(BroadcastBlockDissemination::new(peer_messenger)),
        _ => todo!(),
    }
}

pub async fn block_dissemination_thread(
    id: NodeId,
    dissem_pattern: DissemPattern,
    peer_messenger: Arc<PeerMessenger>,
    mut new_block_recv: mpsc::Receiver<Arc<Block>>,
    block_ready_send: mpsc::Sender<Arc<Block>>,
) {
    log::trace!("Node {id}: Txn Validation stage starting...");

    let block_dissemination_stage = get_block_dissemination(dissem_pattern, peer_messenger);

    loop {
        let new_blk = match new_block_recv.recv().await {
            Some(blk) => blk,
            None => {
                log::error!("Node {id}: new_block pipe closed unexpectedly");
                continue;
            }
        };

        log::trace!("Node {id}: got new block {new_blk:?}");

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
