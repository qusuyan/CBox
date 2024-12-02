mod dummy;
use dummy::DummyTxnValidation;

mod avalanche;
mod bitcoin;

use crate::config::ChainConfig;
use crate::consts::{TXN_BATCH_DELAY_INTERVAL, TXN_BATCH_INTERVAL};
use crate::context::TxnCtx;
use crate::get_report_timer;
use crate::protocol::crypto::Hash;
use crate::protocol::transaction::Txn;
use crate::protocol::CryptoScheme;
use crate::stage::{pass, process_illusion};
use crate::utils::{CopycatError, NodeId};
use crate::vcores::VCoreGroup;

use async_trait::async_trait;

use std::collections::VecDeque;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::time::{Duration, Instant};

use atomic_float::AtomicF64;
use tokio_metrics::TaskMonitor;

#[async_trait]
trait TxnValidation: Send + Sync {
    async fn validate(
        &mut self,
        txn_batch: Vec<(NodeId, (Arc<Txn>, Arc<TxnCtx>))>,
    ) -> Result<Vec<(NodeId, (Arc<Txn>, Arc<TxnCtx>))>, CopycatError>;
}

fn get_txn_validation(id: NodeId, config: ChainConfig) -> Box<dyn TxnValidation> {
    match config {
        ChainConfig::Dummy { .. } | ChainConfig::ChainReplication { .. } => {
            Box::new(DummyTxnValidation::new(id))
        }
        ChainConfig::Bitcoin { config } => bitcoin::new(id, config),
        ChainConfig::Avalanche { config } => avalanche::new(id, config),
    }
}

pub async fn txn_validation_thread(
    id: NodeId,
    config: ChainConfig,
    crypto_scheme: CryptoScheme,
    mut req_recv: mpsc::Receiver<Arc<Txn>>,
    mut peer_txn_recv: mpsc::Receiver<(NodeId, Vec<Arc<Txn>>)>,
    validated_txn_send: mpsc::Sender<Vec<(NodeId, (Arc<Txn>, Arc<TxnCtx>))>>,
    core_group: Arc<VCoreGroup>,
    txns_seen: Arc<DashMap<Hash, bool>>,
    monitor: TaskMonitor,
) {
    pf_info!(id; "txn validation stage starting...");

    const VALIDATION_BATCH_SIZE: usize = 100usize;

    let delay: Arc<AtomicF64> = Arc::new(AtomicF64::new(0f64));
    let mut insert_delay_time = Instant::now() + TXN_BATCH_DELAY_INTERVAL;

    let mut txn_buffer = VecDeque::new();
    let mut txn_batch_time = None;

    let (pending_txns_sender, mut pending_txns_recver) = mpsc::channel(0x100000);

    let mut txn_validation = get_txn_validation(id, config);

    async fn wait_validation_batch(
        batch_len: usize,
        max_batch_size: usize,
        timeout: Option<Instant>,
    ) {
        let notify = tokio::sync::Notify::new();
        if batch_len == 0 || timeout.is_none() {
            notify.notified().await;
        } else if batch_len >= max_batch_size {
            return;
        }
        tokio::time::sleep_until(timeout.unwrap()).await;
    }

    let mut report_timer = get_report_timer();
    let mut self_txns_recved = 0;
    let mut peer_txns_recved = 0;
    let mut txn_batches_validated = 0;
    let mut txns_validated = 0;
    let mut task_interval = monitor.intervals();

    loop {
        tokio::select! {
            new_txn = req_recv.recv(), if txn_buffer.len() < VALIDATION_BATCH_SIZE => {
                let mut txn = match new_txn {
                    Some(txn) => txn,
                    None => {
                        pf_error!(id; "request pipe closed");
                        continue;
                    }
                };

                loop {
                    pf_trace!(id; "got from self new txn {:?}", txn);
                    self_txns_recved += 1;
                    txn_buffer.push_back((id, txn));

                    if txn_buffer.len() >= VALIDATION_BATCH_SIZE {
                        break;
                    }

                    txn = match req_recv.try_recv() {
                        Ok(txn) => txn,
                        Err(e) => match e {
                            TryRecvError::Empty => break,
                            TryRecvError::Disconnected => {
                                pf_error!(id; "request pipe closed");
                                break;
                            }
                        }
                    }
                }

                if txn_batch_time.is_none() {
                    txn_batch_time = Some(Instant::now() + TXN_BATCH_INTERVAL);
                }
            },

            new_peer_txn = peer_txn_recv.recv() => {
                let (src, txns) = match new_peer_txn {
                    Some((src, txn)) => {
                        if src == id {
                            // ignore txns sent by self
                            continue;
                        }
                        (src, txn)
                    },
                    None => {
                        pf_error!(id; "peer_txn pipe closed");
                        continue;
                    },
                };

                pf_trace!(id; "got from peer {} new txns {:?}", src, txns);
                peer_txns_recved += txns.len();
                let txn_batch = txns.into_iter().map(|txn| (src, txn));
                txn_buffer.extend(txn_batch);

                if txn_batch_time.is_none() {
                    txn_batch_time = Some(Instant::now() + TXN_BATCH_INTERVAL);
                }
            }

            _ = wait_validation_batch(txn_buffer.len(), VALIDATION_BATCH_SIZE, txn_batch_time), if txn_batch_time.is_some() => {
                let num_txns_to_drain = std::cmp::min(VALIDATION_BATCH_SIZE, txn_buffer.len());
                let txns_to_validate: Vec<_> = txn_buffer.drain(0..num_txns_to_drain).collect();

                if txn_buffer.len() == 0 {
                    txn_batch_time = None;
                }

                let txn_seen = txns_seen.clone();
                let txn_sender = pending_txns_sender.clone();
                let sem = core_group.clone();
                let delay_pool = delay.clone();

                tokio::spawn(async move {
                    // batch validating txns
                    let _permit = match sem.acquire().await {
                        Ok(permit) => permit,
                        Err(e) => {
                            pf_error!(id; "failed to acquire allowed concurrency: {:?}", e);
                            return;
                        }
                    };

                    let mut correct_txns = vec![];
                    let mut verification_time = 0f64;

                    for (src, txn) in txns_to_validate.into_iter() {
                        let txn_ctx = match TxnCtx::from_txn(&txn) {
                            Ok(ctx) => Arc::new(ctx),
                            Err(_) => continue,
                        };
                        let hash = &txn_ctx.id;

                        // ignore duplicates
                        if txn_seen.contains_key(hash) {
                            continue;
                        }
                        // ignore invalid txns
                        let (valid, vtime) = match txn.validate(crypto_scheme) {
                            Ok(validity) => validity,
                            Err(_) => continue,
                        };
                        verification_time += vtime;
                        txn_seen.insert(*hash, valid);

                        if !valid {
                            continue;
                        }

                        correct_txns.push((src, (txn, txn_ctx)));
                    }

                    process_illusion(Duration::from_secs_f64(verification_time), &delay_pool).await;

                    drop(_permit);

                    if let Err(e) = txn_sender.send(correct_txns).await {
                        pf_error!(id; "failed to send to pending_txns pipe: {:?}", e);
                    }
                });
            }

            txns_with_ctx = pending_txns_recver.recv() => {
                let txns_with_ctx = match txns_with_ctx {
                    Some(batch) => batch,
                    None => {
                        pf_error!(id; "pending_txns closed");
                        continue;
                    }
                };

                let _permit = match core_group.acquire().await {
                    Ok(permit) => permit,
                    Err(e) => {
                        pf_error!(id; "failed to acquire allowed concurrency: {:?}", e);
                        return;
                    }
                };

                let correct_txns = match txn_validation.validate(txns_with_ctx).await {
                    Ok(txns) => txns,
                    Err(e) => {
                        pf_error!(id; "failed to validate txn batch: {:?}", e);
                        continue;
                    }
                };

                drop(_permit);

                txn_batches_validated += 1;
                txns_validated += correct_txns.len();

                if let Err(e) = validated_txn_send.send(correct_txns).await {
                    pf_error!(id; "failed to send to validated_txn pipe: {:?}", e);
                }
            }

            _ = pass(), if Instant::now() > insert_delay_time => {
                // insert delay as appropriate
                let sleep_time = delay.load(Ordering::Relaxed);
                if sleep_time > 0.05 {
                    // doing skipped compute cost
                    let _permit = match core_group.acquire().await {
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
                insert_delay_time = Instant::now() + TXN_BATCH_DELAY_INTERVAL;
            }

            report_val = report_timer.changed() => {
                if let Err(e) = report_val {
                    pf_error!(id; "Waiting for report timeout failed: {}", e);
                }

                // report basic statistics
                pf_info!(id; "In the last minute: self_txns_recved: {}, peer_txns_recved: {}", self_txns_recved, peer_txns_recved);
                pf_info!(id; "In the last minute: txn_batches_validated: {}, txns_validated: {}", txn_batches_validated, txns_validated);

                self_txns_recved = 0;
                peer_txns_recved = 0;
                txn_batches_validated = 0;
                txns_validated = 0;

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
