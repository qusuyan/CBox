mod dummy;
use dummy::DummyCommit;

mod execute;
use execute::ExecuteCommit;

use crate::config::{AptosConfig, ChainConfig};
use crate::consts::COMMIT_DELAY_INTERVAL;
use crate::context::TxnCtx;
use crate::protocol::transaction::Txn;
use crate::stage::{pass, DelayPool};
use crate::vcores::VCoreGroup;
use crate::{get_report_timer, CopycatError, NodeId};

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio_metrics::TaskMonitor;

#[async_trait]
trait Commit: Sync + Send {
    async fn commit(
        &mut self,
        block: Vec<Arc<Txn>>,
        ctx: Vec<Arc<TxnCtx>>,
    ) -> Result<Vec<Arc<Txn>>, CopycatError>;
}

fn get_commit(id: NodeId, config: ChainConfig, delay: Arc<DelayPool>) -> Box<dyn Commit> {
    match config {
        ChainConfig::Dummy { .. } => Box::new(DummyCommit::new()),
        ChainConfig::Bitcoin { .. } => Box::new(DummyCommit::new()), // TODO:
        ChainConfig::Avalanche { .. } => Box::new(DummyCommit::new()), // TODO:
        ChainConfig::ChainReplication { .. } => Box::new(DummyCommit::new()), // TODO:
        ChainConfig::Diem { .. } => Box::new(DummyCommit::new()),    // TODO:
        ChainConfig::Aptos { config } => match config {
            AptosConfig::Basic { config } => {
                if config.commit {
                    Box::new(ExecuteCommit::new(id, delay))
                } else {
                    Box::new(DummyCommit::new())
                }
            }
        },
    }
}

pub async fn commit_thread(
    id: NodeId,
    config: ChainConfig,
    mut commit_recv: mpsc::Receiver<(u64, (Vec<Arc<Txn>>, Vec<Arc<TxnCtx>>))>,
    executed_send: mpsc::Sender<(NodeId, (u64, Vec<Arc<Txn>>))>,
    core_group: Arc<VCoreGroup>,
    monitor: TaskMonitor,
) {
    pf_info!(id; "commit stage starting...");

    let delay = Arc::new(DelayPool::new());
    let mut insert_delay_time = Instant::now() + COMMIT_DELAY_INTERVAL;

    let mut commit_stage = get_commit(id, config, delay);

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

                let (height, (txn_batch, txn_batch_ctx)) = match new_batch {
                    Some(blk) => blk,
                    None => {
                        pf_error!(id; "commit pipe closed unexpectedly");
                        return;
                    }
                };

                pf_debug!(id; "got new txn batch at height {} ({} txns)", height, txn_batch.len());

                let correct_batch = match commit_stage.commit(txn_batch, txn_batch_ctx).await {
                    Ok(batch) => batch,
                    Err(e) => {
                        pf_error!(id; "failed to commit: {:?}", e);
                        continue;
                    }
                };

                drop(_permit);

                if let Err(e) = executed_send.send((id, (height, correct_batch))).await {
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
