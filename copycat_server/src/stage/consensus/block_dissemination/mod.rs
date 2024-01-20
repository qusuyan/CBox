mod broadcast;
use broadcast::BroadcastBlockDissemination;

use crate::{config::Config, peers::PeerMessenger};
use copycat_protocol::block::Block;
use copycat_protocol::DissemPattern;
use copycat_utils::{CopycatError, NodeId};

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::mpsc;

#[async_trait]
pub trait BlockDissemination: Sync + Send {
    async fn disseminate(&self, src: NodeId, block: &Block) -> Result<(), CopycatError>;
}

fn get_block_dissemination(
    id: NodeId,
    dissem_pattern: DissemPattern,
    _config: Config,
    peer_messenger: Arc<PeerMessenger>,
) -> Box<dyn BlockDissemination> {
    match dissem_pattern {
        DissemPattern::Broadcast => Box::new(BroadcastBlockDissemination::new(id, peer_messenger)),
        _ => todo!(),
    }
}

pub async fn block_dissemination_thread(
    id: NodeId,
    dissem_pattern: DissemPattern,
    config: Config,
    peer_messenger: Arc<PeerMessenger>,
    mut new_block_recv: mpsc::Receiver<(NodeId, Vec<Arc<Block>>)>,
    block_ready_send: mpsc::Sender<Vec<Arc<Block>>>,
) {
    log::info!("block dissemination stage starting...");

    let block_dissemination_stage =
        get_block_dissemination(id, dissem_pattern, config, peer_messenger);

    loop {
        let (src, new_tail) = match new_block_recv.recv().await {
            Some(blk) => blk,
            None => {
                log::error!("new_block pipe closed unexpectedly");
                return;
            }
        };

        // only disseminate the last block
        let new_blk = match new_tail.last() {
            Some(blk) => blk,
            None => continue,
        };

        log::debug!("got new block {new_blk:?}");

        if let Err(e) = block_dissemination_stage.disseminate(src, &new_blk).await {
            log::error!("failed to disseminate new block: {e:?}");
            continue;
        }

        if let Err(e) = block_ready_send.send(new_tail).await {
            log::error!("failed to send to block_ready pipe: {e:?}");
            continue;
        }
    }
}
