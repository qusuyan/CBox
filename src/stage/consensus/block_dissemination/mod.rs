mod broadcast;
use broadcast::BroadcastBlockDissemination;

mod gossip;
use gossip::GossipBlockDissemination;

mod sampling;
use sampling::SamplingBlockDissemination;

mod linear;
use linear::LinearBlockDissemination;
use tokio_metrics::TaskMonitor;

use crate::config::Config;
use crate::context::BlkCtx;
use crate::peers::PeerMessenger;
use crate::protocol::block::Block;
use crate::protocol::DissemPattern;
use crate::utils::{CopycatError, NodeId};

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::{mpsc, Semaphore};
use tokio::time::{Duration, Instant};

use atomic_float::AtomicF64;
use std::sync::atomic::Ordering;

#[async_trait]
trait BlockDissemination: Sync + Send {
    async fn disseminate(&self, src: NodeId, block: &Block) -> Result<(), CopycatError>;
}

fn get_block_dissemination(
    id: NodeId,
    config: Config,
    peer_messenger: Arc<PeerMessenger>,
) -> Box<dyn BlockDissemination> {
    match config.get_blk_dissem() {
        DissemPattern::Broadcast => Box::new(BroadcastBlockDissemination::new(id, peer_messenger)),
        DissemPattern::Gossip => Box::new(GossipBlockDissemination::new(id, peer_messenger)),
        DissemPattern::Sample => {
            Box::new(SamplingBlockDissemination::new(id, peer_messenger, config))
        }
        DissemPattern::Linear => {
            Box::new(LinearBlockDissemination::new(id, peer_messenger, config))
        }
        DissemPattern::Passthrough => unimplemented!(),
    }
}

pub async fn block_dissemination_thread(
    id: NodeId,
    config: Config,
    peer_messenger: Arc<PeerMessenger>,
    mut new_block_recv: mpsc::Receiver<(NodeId, Vec<(Arc<Block>, Arc<BlkCtx>)>)>,
    block_ready_send: mpsc::Sender<(NodeId, Vec<(Arc<Block>, Arc<BlkCtx>)>)>,
    concurrency: Arc<Semaphore>,
    monitor: TaskMonitor,
) {
    pf_info!(id; "block dissemination stage starting...");

    let delay = Arc::new(AtomicF64::new(0f64));
    let insert_delay_interval = Duration::from_millis(50);
    let mut insert_delay_time = Instant::now() + insert_delay_interval;

    let block_dissemination_stage = get_block_dissemination(id, config, peer_messenger);

    let mut report_timeout = Instant::now() + Duration::from_secs(60);
    let mut task_interval = monitor.intervals();

    loop {
        tokio::select! {
            new_blk = new_block_recv.recv() => {
                // for serializing blocks sent
                let _permit = match concurrency.acquire().await {
                    Ok(permit) => permit,
                    Err(e) => {
                        pf_error!(id; "failed to acquire allowed concurrency: {:?}", e);
                        continue;
                    }
                };

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

                drop(_permit);

                if let Err(e) = block_ready_send.send((src, new_tail)).await {
                    pf_error!(id; "failed to send to block_ready pipe: {:?}", e);
                    continue;
                }
            },
            _ = tokio::time::sleep_until(insert_delay_time) => {
                // insert delay as appropriate
                let sleep_time = delay.load(Ordering::Relaxed);
                if sleep_time > 0.05 {
                    // doing skipped compute cost
                    let _permit = match concurrency.acquire().await {
                        Ok(permit) => permit,
                        Err(e) => {
                            pf_error!(id; "failed to acquire allowed concurrency: {:?}", e);
                            continue;
                        }
                    };
                    tokio::time::sleep(Duration::from_secs_f64(sleep_time)).await;
                    delay.store(0f64, Ordering::Relaxed);
                } else {
                    tokio::task::yield_now().await;
                }
                insert_delay_time = Instant::now() + insert_delay_interval;
            }
            _ = tokio::time::sleep_until(report_timeout) => {
                let metrics = task_interval.next().unwrap();
                let sched_count = metrics.total_scheduled_count;
                let mean_sched_dur = metrics.mean_scheduled_duration().as_secs_f64();
                let poll_count = metrics.total_poll_count;
                let mean_poll_dur = metrics.mean_poll_duration().as_secs_f64();
                pf_info!(id; "In the last minute: sched_count: {}, mean_sched_dur: {} s, poll_count: {}, mean_poll_dur: {} s", sched_count, mean_sched_dur, poll_count, mean_poll_dur);
                // reset report time
                report_timeout = Instant::now() + Duration::from_secs(60);
            }
        }
    }
}
