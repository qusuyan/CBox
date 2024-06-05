mod broadcast;
use broadcast::BroadcastBlockDissemination;

mod gossip;
use gossip::GossipBlockDissemination;

mod sampling;
use sampling::SamplingBlockDissemination;

use crate::config::Config;
use crate::context::BlkCtx;
use crate::peers::PeerMessenger;
use crate::protocol::block::Block;
use crate::protocol::DissemPattern;
use crate::utils::{CopycatError, NodeId};

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{Duration, Instant};

use atomic_float::AtomicF64;
use std::sync::atomic::Ordering;

#[async_trait]
pub trait BlockDissemination: Sync + Send {
    async fn disseminate(&self, src: NodeId, block: &Block) -> Result<(), CopycatError>;
}

fn get_block_dissemination(
    id: NodeId,
    dissem_pattern: DissemPattern,
    config: Config,
    peer_messenger: Arc<PeerMessenger>,
) -> Box<dyn BlockDissemination> {
    match dissem_pattern {
        DissemPattern::Broadcast => Box::new(BroadcastBlockDissemination::new(id, peer_messenger)),
        DissemPattern::Gossip => Box::new(GossipBlockDissemination::new(id, peer_messenger)),
        DissemPattern::Sample => {
            Box::new(SamplingBlockDissemination::new(id, peer_messenger, config))
        }
    }
}

pub async fn block_dissemination_thread(
    id: NodeId,
    dissem_pattern: DissemPattern,
    config: Config,
    peer_messenger: Arc<PeerMessenger>,
    mut new_block_recv: mpsc::Receiver<(NodeId, Vec<(Arc<Block>, Arc<BlkCtx>)>)>,
    block_ready_send: mpsc::Sender<(NodeId, Vec<(Arc<Block>, Arc<BlkCtx>)>)>,
) {
    pf_info!(id; "block dissemination stage starting...");

    let delay = Arc::new(AtomicF64::new(0f64));

    let block_dissemination_stage =
        get_block_dissemination(id, dissem_pattern, config, peer_messenger);

    let mut report_timeout = Instant::now() + Duration::from_secs(60);

    loop {
        tokio::select! {
            new_blk = new_block_recv.recv() => {
                let (src, new_tail) = match new_blk {
                    Some(blk) => blk,
                    None => {
                        pf_error!(id; "new_block pipe closed unexpectedly");
                        return;
                    }
                };

                // only disseminate the last block
                let (new_blk, _) = match new_tail.last() {
                    Some(blk) => blk,
                    None => continue,
                };

                pf_debug!(id; "got new block {:?}", new_blk);

                if let Err(e) = block_dissemination_stage.disseminate(src, &new_blk).await {
                    pf_error!(id; "failed to disseminate new block: {:?}", e);
                    continue;
                }

                if let Err(e) = block_ready_send.send((src, new_tail)).await {
                    pf_error!(id; "failed to send to block_ready pipe: {:?}", e);
                    continue;
                }
            },
            _ = tokio::time::sleep_until(report_timeout) => {
                // reset report time
                report_timeout = Instant::now() + Duration::from_secs(60);
            }
        }

        // insert delay as appropriate
        let sleep_time = delay.load(Ordering::Relaxed);
        if sleep_time > 0.05 {
            tokio::time::sleep(Duration::from_secs_f64(sleep_time)).await;
            delay.store(0f64, Ordering::Relaxed);
        }
    }
}
