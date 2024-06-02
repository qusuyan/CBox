mod broadcast;
use broadcast::BroadcastBlockDissemination;

mod gossip;
use gossip::GossipBlockDissemination;

mod sampling;
use sampling::SamplingBlockDissemination;
use tokio::time::{Duration, Instant};

use crate::config::Config;
use crate::context::BlkCtx;
use crate::peers::PeerMessenger;
use crate::protocol::block::Block;
use crate::protocol::DissemPattern;
use crate::utils::{CopycatError, NodeId};

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_metrics::TaskMonitor;

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
    task_monitor: TaskMonitor,
) {
    pf_info!(id; "block dissemination stage starting...");

    let block_dissemination_stage =
        get_block_dissemination(id, dissem_pattern, config, peer_messenger);

    let mut report_timeout = Instant::now() + Duration::from_secs(60);
    let mut metric_intervals = task_monitor.intervals();

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
                // report task monitor statistics
                let metrics = match metric_intervals.next() {
                    Some(metrics) => metrics,
                    None => {
                        pf_error!(id; "failed to fetch metrics for the last minute");
                        continue;
                    }
                };
                let poll_duration = metrics.mean_poll_duration().as_secs_f64() * 1000f64;
                let poll_count = metrics.total_poll_count;
                pf_info!(id; "In the last minute: mean poll duration: {} ms, poll_count: {}", poll_duration, poll_count);

                // reset report time
                report_timeout = Instant::now() + Duration::from_secs(60);
            }
        }
    }
}
