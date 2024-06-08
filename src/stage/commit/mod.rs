mod dummy;
use dummy::DummyCommit;

use crate::config::Config;
use crate::protocol::transaction::Txn;
use crate::{CopycatError, NodeId};

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::{mpsc, Semaphore};
use tokio::time::{Duration, Instant};

use atomic_float::AtomicF64;
use std::sync::atomic::Ordering;

#[async_trait]
pub trait Commit: Sync + Send {
    async fn commit(&self, block: &Vec<Arc<Txn>>) -> Result<(), CopycatError>;
}

pub fn get_commit(_id: NodeId, config: Config) -> Box<dyn Commit> {
    match config {
        Config::Dummy => Box::new(DummyCommit::new()),
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
) {
    pf_info!(id; "commit stage starting...");

    let delay = Arc::new(AtomicF64::new(0f64));
    let insert_delay_interval = Duration::from_millis(50);
    let mut insert_delay_time = Instant::now() + insert_delay_interval;

    let commit_stage = get_commit(id, config);

    let mut report_timeout = Instant::now() + Duration::from_secs(60);

    loop {
        tokio::select! {
            new_batch = commit_recv.recv() => {
                let _ = match concurrency.acquire().await {
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

                if let Err(e) = executed_send.send((height, txn_batch)).await {
                    pf_error!(id; "failed to send committed txns: {:?}", e);
                    continue;
                }
            },
            _ = tokio::time::sleep_until(insert_delay_time) => {
                // insert delay as appropriate
                let sleep_time = delay.load(Ordering::Relaxed);
                if sleep_time > 0.05 {
                    let _ = match concurrency.acquire().await {
                        Ok(permit) => permit,
                        Err(e) => {
                            pf_error!(id; "failed to acquire allowed concurrency: {:?}", e);
                            continue;
                        }
                    };
                    tokio::time::sleep(Duration::from_secs_f64(sleep_time)).await;
                    delay.store(0f64, Ordering::Relaxed);
                }
                insert_delay_time = Instant::now() + insert_delay_interval;
            }
            _ = tokio::time::sleep_until(report_timeout) => {
                // reset report time
                report_timeout = Instant::now() + Duration::from_secs(60);
            }
        }
    }
}
