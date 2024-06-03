mod broadcast;
use broadcast::BroadcastTxnDissemination;

mod gossip;
use gossip::GossipTxnDissemination;

use crate::context::TxnCtx;
use crate::protocol::transaction::Txn;
use crate::protocol::DissemPattern;
use crate::utils::{CopycatError, NodeId};
use crate::{config::Config, peers::PeerMessenger};

use async_trait::async_trait;
use get_size::GetSize;

use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{Duration, Instant};
use tokio_metrics::TaskMonitor;

use atomic_float::AtomicF64;
use std::sync::atomic::Ordering;

#[async_trait]
pub trait TxnDissemination: Send + Sync {
    async fn disseminate(&self, txn_batch: &Vec<(u64, Arc<Txn>)>) -> Result<(), CopycatError>;
}

fn get_txn_dissemination(
    id: NodeId,
    dissem_pattern: DissemPattern,
    _config: Config,
    peer_messenger: Arc<PeerMessenger>,
) -> Box<dyn TxnDissemination> {
    match dissem_pattern {
        DissemPattern::Broadcast => Box::new(BroadcastTxnDissemination::new(id, peer_messenger)),
        DissemPattern::Gossip => Box::new(GossipTxnDissemination::new(id, peer_messenger)),
        DissemPattern::Sample => Box::new(BroadcastTxnDissemination::new(id, peer_messenger)), // TODO: use gossip for now
    }
}

pub async fn txn_dissemination_thread(
    id: NodeId,
    dissem_pattern: DissemPattern,
    config: Config,
    enabled: bool,
    peer_messenger: Arc<PeerMessenger>,
    mut validated_txn_recv: mpsc::Receiver<Vec<(NodeId, (Arc<Txn>, Arc<TxnCtx>))>>,
    txn_ready_send: mpsc::Sender<(Arc<Txn>, Arc<TxnCtx>)>,
    task_monitor: TaskMonitor,
) {
    pf_info!(id; "txn dissemination stage starting...");

    let delay = Arc::new(AtomicF64::new(0f64));

    let txn_dissemination_stage = get_txn_dissemination(id, dissem_pattern, config, peer_messenger);
    let mut batch = vec![];
    let mut txn_dissem_time = Instant::now() + Duration::from_millis(100);

    async fn wait_send_batch(batch: &Vec<(u64, (Arc<Txn>, Arc<TxnCtx>))>, timeout: Instant) {
        let notify = tokio::sync::Notify::new();
        if batch.len() == 0 {
            notify.notified().await;
        } else if batch.get_size() > 0x100000 {
            return;
        }
        tokio::time::sleep_until(timeout).await;
    }

    let mut report_timeout = Instant::now() + Duration::from_secs(60);
    let mut metric_intervals = task_monitor.intervals();

    loop {
        tokio::select! {
            new_txn = validated_txn_recv.recv() => {
                let mut txn_batch = match new_txn {
                    Some(txn) => txn,
                    None => {
                        pf_error!(id; "validated_txn pipe closed unexpectedly");
                        return;
                    }
                };

                pf_trace!(id; "got new txn batch {:?}", txn_batch);
                batch.append(&mut txn_batch);
            }

            _ = wait_send_batch(&batch, txn_dissem_time) => {
                let send_batch: Vec<(u64, (Arc<Txn>, Arc<TxnCtx>))> = batch.drain(0..).collect();
                let txns = send_batch.iter().map(|(src, (txn, _))| (*src, txn.clone())).collect();
                if enabled {
                    if let Err(e) = txn_dissemination_stage.disseminate(&txns).await {
                        pf_error!(id; "failed to disseminate txn: {:?}", e);
                        continue;
                    }
                }

                for (_, txn) in send_batch.into_iter() {
                    if let Err(e) = txn_ready_send.send(txn).await {
                        pf_error!(id; "failed to send to txn_ready pipe: {:?}", e);
                        continue;
                    }
                }

                txn_dissem_time = Instant::now() + Duration::from_millis(100);
            }
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

        // insert delay as appropriate
        let sleep_time = delay.load(Ordering::Relaxed);
        if sleep_time > 0.05 {
            tokio::time::sleep(Duration::from_secs_f64(sleep_time)).await;
            delay.store(0f64, Ordering::Relaxed);
        }
    }
}
