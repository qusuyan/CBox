mod dummy;
use dummy::DummyCommit;
use tokio::time::{Duration, Instant};

use crate::config::Config;
use crate::protocol::transaction::Txn;
use crate::{CopycatError, NodeId};

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_metrics::TaskMonitor;

#[async_trait]
pub trait Commit: Sync + Send {
    async fn commit(&self, block: &Vec<Arc<Txn>>) -> Result<(), CopycatError>;
}

pub fn get_commit(_id: NodeId, config: Config) -> Box<dyn Commit> {
    match config {
        Config::Dummy => Box::new(DummyCommit::new()),
        Config::Bitcoin { .. } => Box::new(DummyCommit::new()), // TODO:
        Config::Avalanche { .. } => Box::new(DummyCommit::new()), // TODO:
    }
}

pub async fn commit_thread(
    id: NodeId,
    config: Config,
    mut commit_recv: mpsc::Receiver<(u64, Vec<Arc<Txn>>)>,
    executed_send: mpsc::Sender<(u64, Vec<Arc<Txn>>)>,
    task_monitor: TaskMonitor,
) {
    pf_info!(id; "commit stage starting...");

    let commit_stage = get_commit(id, config);

    let mut report_timeout = Instant::now() + Duration::from_secs(60);
    let mut metric_intervals = task_monitor.intervals();

    loop {
        tokio::select! {
            new_batch = commit_recv.recv() => {
                let (height, txn_batch) = match new_batch {
                    Some(blk) => blk,
                    None => {
                        pf_error!(id; "commit pipe closed unexpectedly");
                        return;
                    }
                };

                pf_debug!(id; "got new txn batch ({})", txn_batch.len());

                if let Err(e) = commit_stage.commit(&txn_batch).await {
                    pf_error!(id; "failed to commit: {:?}", e);
                    continue;
                }

                if let Err(e) = executed_send.send((height, txn_batch)).await {
                    pf_error!(id; "failed to send committed txns: {:?}", e);
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
                let sched_duration = metrics.mean_scheduled_duration().as_secs_f64() * 1000f64;
                let sched_count = metrics.total_scheduled_count;
                pf_info!(id; "In the last minute: mean scheduled duration: {} ms, scheduled count: {}", sched_duration, sched_count);

                // reset report time
                report_timeout = Instant::now() + Duration::from_secs(60);
            }
        }
    }
}
