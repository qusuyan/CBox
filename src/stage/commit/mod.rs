mod dummy;
use dummy::DummyCommit;
use tokio_metrics::TaskMonitor;

use crate::config::ChainConfig;
use crate::consts::COMMIT_DELAY_INTERVAL;
use crate::protocol::transaction::Txn;
use crate::stage::{pass, DelayPool};
use crate::vcores::VCoreGroup;
use crate::{get_report_timer, CopycatError, NodeId};

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::Instant;

#[async_trait]
trait Commit: Sync + Send {
    async fn commit(&self, block: &Vec<Arc<Txn>>) -> Result<(), CopycatError>;
}

fn get_commit(_id: NodeId, config: ChainConfig) -> Box<dyn Commit> {
    match config {
        ChainConfig::Dummy { .. } => Box::new(DummyCommit::new()),
        ChainConfig::Bitcoin { .. } => Box::new(DummyCommit::new()), // TODO:
        ChainConfig::Avalanche { .. } => Box::new(DummyCommit::new()), // TODO:
        ChainConfig::ChainReplication { .. } => Box::new(DummyCommit::new()), // TODO:
        ChainConfig::Diem { .. } => Box::new(DummyCommit::new()),    // TODO:
    }
}

pub async fn commit_thread(
    id: NodeId,
    config: ChainConfig,
    mut commit_recv: mpsc::Receiver<(u64, Vec<Arc<Txn>>)>,
    executed_send: mpsc::Sender<(NodeId, (u64, Vec<Arc<Txn>>))>,
    core_group: Arc<VCoreGroup>,
    monitor: TaskMonitor,
) {
    pf_info!(id; "commit stage starting...");

    let _delay = Arc::new(DelayPool::new());
    let mut insert_delay_time = Instant::now() + COMMIT_DELAY_INTERVAL;

    let commit_stage = get_commit(id, config);

    let mut report_timer = get_report_timer();
    let mut task_interval = monitor.intervals();

    loop {
        tokio::select! {
            new_batch = commit_recv.recv() => {
                // commit transaction
                let _permit = match core_group.acquire().await {
                    Ok(permit) => permit,
                    Err(e) => {
                        pf_error!(id; "failed to acquire allowed concurrency: {:?}", e);
                        continue;
                    }
                };

                let (height, txn_batch) = match new_batch {
                    Some(blk) => blk,
                    None => {
                        pf_error!(id; "commit pipe closed unexpectedly");
                        return;
                    }
                };

                pf_debug!(id; "got new txn batch at height {} ({} txns)", height, txn_batch.len());

                if let Err(e) = commit_stage.commit(&txn_batch).await {
                    pf_error!(id; "failed to commit: {:?}", e);
                    continue;
                }

                drop(_permit);

                if let Err(e) = executed_send.send((id, (height, txn_batch))).await {
                    pf_error!(id; "failed to send committed txns: {:?}", e);
                    continue;
                }
            },

            _ = pass(), if Instant::now() > insert_delay_time => {
                tokio::task::yield_now().await;
                insert_delay_time = Instant::now() + COMMIT_DELAY_INTERVAL;
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
