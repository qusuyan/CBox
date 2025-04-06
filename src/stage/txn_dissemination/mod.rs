mod broadcast;
use broadcast::BroadcastTxnDissemination;

mod gossip;
use gossip::GossipTxnDissemination;

mod passthrough;
use passthrough::PassthroughTxnDissemination;
use tokio_metrics::TaskMonitor;

use crate::consts::{TXM_DISSEM_DELAY_INTERVAL, TXN_DISSEM_INTERVAL};
use crate::context::TxnCtx;
use crate::peers::PeerMessenger;
use crate::protocol::transaction::Txn;
use crate::protocol::DissemPattern;
use crate::stage::{pass, DelayPool};
use crate::utils::{CopycatError, NodeId};
use crate::vcores::VCoreGroup;
use crate::{get_report_timer, ChainConfig};

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::Instant;

#[async_trait]
trait TxnDissemination: Send + Sync {
    async fn disseminate(&self, txn_batch: &Vec<(u64, Arc<Txn>)>) -> Result<(), CopycatError>;
}

fn get_txn_dissemination(
    id: NodeId,
    enabled: bool,
    config: ChainConfig,
    peer_messenger: Arc<PeerMessenger>,
) -> Box<dyn TxnDissemination> {
    if !enabled {
        return Box::new(PassthroughTxnDissemination::new());
    }

    match config.get_txn_dissem() {
        DissemPattern::Broadcast => Box::new(BroadcastTxnDissemination::new(id, peer_messenger)),
        DissemPattern::Gossip => Box::new(GossipTxnDissemination::new(id, peer_messenger)),
        DissemPattern::Sample { .. } => todo!(),
        DissemPattern::Passthrough => Box::new(PassthroughTxnDissemination::new()),
        DissemPattern::Linear { .. } => todo!(),
        DissemPattern::Narwhal { .. } => unreachable!(),
    }
}

pub async fn txn_dissemination_thread(
    id: NodeId,
    config: ChainConfig,
    enabled: bool,
    peer_messenger: Arc<PeerMessenger>,
    mut validated_txn_recv: mpsc::Receiver<Vec<(NodeId, (Arc<Txn>, Arc<TxnCtx>))>>,
    txn_ready_send: mpsc::Sender<Vec<(Arc<Txn>, Arc<TxnCtx>)>>,
    core_group: Arc<VCoreGroup>,
    monitor: TaskMonitor,
) {
    pf_info!(id; "txn dissemination stage starting...");

    const DISSEM_BATCH_SIZE: usize = 2000usize;

    let _delay_pool = Arc::new(DelayPool::new());
    let mut insert_delay_time = Instant::now() + TXM_DISSEM_DELAY_INTERVAL;

    let txn_dissemination_stage = get_txn_dissemination(id, enabled, config, peer_messenger);
    let mut batch = vec![];
    let mut txn_dissem_time = None;

    async fn wait_send_batch(batch_len: usize, max_batch_len: usize, timeout: Option<Instant>) {
        let notify = tokio::sync::Notify::new();
        if batch_len == 0 || timeout.is_none() {
            notify.notified().await;
        } else if batch_len >= max_batch_len {
            return;
        }
        tokio::time::sleep_until(timeout.unwrap()).await;
    }

    let mut report_timer = get_report_timer();
    let mut task_interval = monitor.intervals();

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
                if txn_dissem_time.is_none() {
                    txn_dissem_time = Some(Instant::now() + TXN_DISSEM_INTERVAL);
                }
            }

            _ = wait_send_batch(batch.len(), DISSEM_BATCH_SIZE, txn_dissem_time), if txn_dissem_time.is_some() => {
                // serializing the list of txns to be disseminated
                let _permit = match core_group.acquire().await {
                    Ok(permit) => permit,
                    Err(e) => {
                        pf_error!(id; "failed to acquire allowed concurrency: {:?}", e);
                        continue;
                    }
                };

                let send_batch_size = std::cmp::min(DISSEM_BATCH_SIZE, batch.len());
                let send_batch: Vec<(u64, (Arc<Txn>, Arc<TxnCtx>))> = batch.drain(0..send_batch_size).collect();
                let txns = send_batch.iter().map(|(src, (txn, _))| (*src, txn.clone())).collect();
                if let Err(e) = txn_dissemination_stage.disseminate(&txns).await {
                    pf_error!(id; "failed to disseminate txn: {:?}", e);
                    continue;
                }

                let batched_txns = send_batch.into_iter().map(|(_, txn_with_ctx)| txn_with_ctx).collect();
                if batch.len() == 0 {
                    txn_dissem_time = None;
                }

                drop(_permit);

                if let Err(e) = txn_ready_send.send(batched_txns).await {
                    pf_error!(id; "failed to send to txn_ready pipe: {:?}", e);
                    continue;
                }

            }

            _ = pass(), if Instant::now() > insert_delay_time => {
                tokio::task::yield_now().await;
                insert_delay_time = Instant::now() + TXM_DISSEM_DELAY_INTERVAL;
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
