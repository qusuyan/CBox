mod dummy;
use dummy::DummyCommit;
use tokio_metrics::TaskMonitor;

use crate::config::Config;
use crate::protocol::transaction::Txn;
use crate::{CopycatError, NodeId};
use crate::stage::pass;

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::{mpsc, Semaphore};
use tokio::time::{Duration, Instant};

use atomic_float::AtomicF64;
use std::sync::atomic::Ordering;

#[async_trait]
trait Commit: Sync + Send {
    async fn commit(&self, block: &Vec<Arc<Txn>>) -> Result<(), CopycatError>;
}

fn get_commit(_id: NodeId, config: Config) -> Box<dyn Commit> {
    match config {
        Config::Dummy { .. } => Box::new(DummyCommit::new()),
        Config::Bitcoin { .. } => Box::new(DummyCommit::new()), // TODO:
        Config::Avalanche { .. } => Box::new(DummyCommit::new()), // TODO:
        Config::ChainReplication { .. } => Box::new(DummyCommit::new()), // TODO:
    }
}

pub async fn commit_thread(
    id: NodeId,
    config: Config,
    mut commit_recv: mpsc::Receiver<(u64, Vec<Arc<Txn>>)>,
    executed_send: mpsc::Sender<(u64, Vec<Arc<Txn>>)>,
    concurrency: Arc<Semaphore>,
    monitor: TaskMonitor,
) {
    pf_info!(id; "commit stage starting...");

    const INSERT_DELAY_INTERVAL: Duration = Duration::from_millis(50);
    let delay = Arc::new(AtomicF64::new(0f64));
    let mut insert_delay_time = Instant::now() + INSERT_DELAY_INTERVAL;

    let commit_stage = get_commit(id, config);

    const REPORT_TIME_INTERVAL: Duration = Duration::from_secs(60);
    let mut report_timeout = Instant::now() + REPORT_TIME_INTERVAL;
    let mut task_interval = monitor.intervals();

    loop {
        tokio::select! {
            new_batch = commit_recv.recv() => {
                // commit transaction
                let _permit = match concurrency.acquire().await {
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

                pf_debug!(id; "got new txn batch ({})", txn_batch.len());

                if let Err(e) = commit_stage.commit(&txn_batch).await {
                    pf_error!(id; "failed to commit: {:?}", e);
                    continue;
                }

                drop(_permit);

                if let Err(e) = executed_send.send((height, txn_batch)).await {
                    pf_error!(id; "failed to send committed txns: {:?}", e);
                    continue;
                }
            },
            _ = pass(), if Instant::now() > insert_delay_time => {
                // insert delay as appropriate
                let sleep_time = delay.load(Ordering::Relaxed);
                if sleep_time > 0.05 {
                    // doing aggregated compute cost
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
                insert_delay_time = Instant::now() + INSERT_DELAY_INTERVAL;
            }
            _ = tokio::time::sleep_until(report_timeout) => {
                let metrics = task_interval.next().unwrap();
                let sched_count = metrics.total_scheduled_count;
                let mean_sched_dur = metrics.mean_scheduled_duration().as_secs_f64();
                let poll_count = metrics.total_poll_count;
                let mean_poll_dur = metrics.mean_poll_duration().as_secs_f64();
                pf_info!(id; "In the last minute: sched_count: {}, mean_sched_dur: {} s, poll_count: {}, mean_poll_dur: {} s", sched_count, mean_sched_dur, poll_count, mean_poll_dur);
                // reset report time
                report_timeout = Instant::now() + REPORT_TIME_INTERVAL;
            }
        }
    }
}
