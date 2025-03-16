mod dummy;
use dummy::DummyPacemaker;

mod fixed_inflight_blk;
use fixed_inflight_blk::FixedInflightBlkPacemaker;

pub mod diem;
use diem::DiemPacemaker;

use crate::config::AvalancheConfig;
use crate::consts::PACE_DELAY_INTERVAL;
use crate::get_report_timer;
use crate::stage::{pass, DelayPool};
use crate::utils::{CopycatError, NodeId};
use crate::vcores::VCoreGroup;
use crate::{config::ChainConfig, peers::PeerMessenger};

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio_metrics::TaskMonitor;

#[async_trait]
trait Pacemaker: Sync + Send {
    async fn wait_to_propose(&self);
    async fn get_propose_msg(&mut self) -> Result<Vec<u8>, CopycatError>;
    async fn handle_feedback(&mut self, feedback: Vec<u8>) -> Result<(), CopycatError>;
    async fn handle_peer_msg(&mut self, src: NodeId, msg: Vec<u8>) -> Result<(), CopycatError>;
}

fn get_pacemaker(
    id: NodeId,
    config: ChainConfig,
    peer_messenger: Arc<PeerMessenger>,
) -> Box<dyn Pacemaker> {
    match config {
        ChainConfig::Dummy { .. } => Box::new(DummyPacemaker::new()),
        ChainConfig::Bitcoin { .. } => Box::new(DummyPacemaker::new()), // TODO
        ChainConfig::Avalanche { config } => match config {
            AvalancheConfig::Basic { config } => {
                Box::new(FixedInflightBlkPacemaker::new(config.max_inflight_blk))
            }
            AvalancheConfig::Blizzard { config } => {
                Box::new(FixedInflightBlkPacemaker::new(config.max_inflight_blk))
            }
            AvalancheConfig::VoteNo { .. } => Box::new(DummyPacemaker::new()),
        },
        ChainConfig::ChainReplication { .. } => Box::new(DummyPacemaker::new()), // TODO,
        ChainConfig::Diem { .. } => Box::new(DiemPacemaker::new(id, peer_messenger)),
    }
}

pub async fn pacemaker_thread(
    id: NodeId,
    config: ChainConfig,
    peer_messenger: Arc<PeerMessenger>,
    mut peer_pmaker_recv: mpsc::Receiver<(NodeId, Vec<u8>)>,
    mut pmaker_feedback_recv: mpsc::Receiver<Vec<u8>>,
    should_propose_send: mpsc::Sender<Vec<u8>>,
    core_group: Arc<VCoreGroup>,
    monitor: TaskMonitor,
) {
    pf_info!(id; "pacemaker starting...");

    let _delay_pool = Arc::new(DelayPool::new());
    let mut insert_delay_time = Instant::now() + PACE_DELAY_INTERVAL;

    let mut pmaker = get_pacemaker(id, config, peer_messenger);

    let mut report_timer = get_report_timer();
    let mut task_interval = monitor.intervals();

    loop {
        tokio::select! {
            _ = pmaker.wait_to_propose() => {
                // pmaker logic to decide if current node can propose new block
                let _permit = match core_group.acquire().await {
                    Ok(permit) => permit,
                    Err(e) => {
                        pf_error!(id; "failed to acquire allowed concurrency: {:?}", e);
                        continue;
                    }
                };

                let propose_msg = match pmaker.get_propose_msg().await {
                    Ok(msg) => msg,
                    Err(e) => {
                        pf_error!(id; "error waiting to propose: {:?}", e);
                        continue;
                    }
                };

                pf_debug!(id; "sending propose msg {:?}", propose_msg);

                drop(_permit);

                if let Err(e) = should_propose_send.send(propose_msg).await {
                    pf_error!(id; "failed to send to should_propose pipe: {:?}", e);
                    continue;
                }
            },

            feedback_msg = pmaker_feedback_recv.recv() => {
                // getting feedback from decide stage and update internal states
                let _permit = match core_group.acquire().await {
                    Ok(permit) => permit,
                    Err(e) => {
                        pf_error!(id; "failed to acquire allowed concurrency: {:?}", e);
                        continue;
                    }
                };

                let feedback = match feedback_msg {
                    Some(msg) => msg,
                    None => {
                        pf_error!(id; "pmaker_feedback pipe closed unexpectedly");
                        return;
                    }
                };

                pf_debug!(id; "got feedback from decide stage");
                if let Err(e) = pmaker.handle_feedback(feedback).await {
                    pf_error!(id; "failed to handle feedback: {:?}", e);
                    continue;
                }
            }

            peer_msg = peer_pmaker_recv.recv() => {
                // handle pmaker messages from peers
                let _permit = match core_group.acquire().await {
                    Ok(permit) => permit,
                    Err(e) => {
                        pf_error!(id; "failed to acquire allowed concurrency: {:?}", e);
                        continue;
                    }
                };

                let (src, msg) = match peer_msg {
                    Some(msg) => msg,
                    None => {
                        pf_error!(id; "peer_pmaker pipe closed unexpectedly");
                        return;
                    }
                };

                pf_debug!(id; "got peer pmaker msg from {}", src);
                if let Err(e) = pmaker.handle_peer_msg(src, msg).await {
                    pf_error!(id; "failed to handle peer msg: {:?}", e);
                    continue;
                }
            }

            _ = pass(), if Instant::now() > insert_delay_time => {
                tokio::task::yield_now().await;
                insert_delay_time = Instant::now() + PACE_DELAY_INTERVAL;
            }

            report_val = report_timer.changed() => {
                if let Err(e) = report_val {
                    pf_error!(id; "Waiting for report timeout failed: {}", e);
                }
                let metrics = task_interval.next().unwrap();
                let sched_count = metrics.total_scheduled_count;
                let mean_sched_dur = metrics.mean_scheduled_duration().as_secs_f64();
                let poll_count = metrics.total_poll_count;
                let mean_poll_dur = metrics.mean_poll_duration().as_secs_f64();
                pf_info!(id; "In the last minute: sched_count: {}, mean_sched_dur: {} s, poll_count: {}, mean_poll_dur: {} s", sched_count, mean_sched_dur, poll_count, mean_poll_dur);
            }
        }
    }
}
