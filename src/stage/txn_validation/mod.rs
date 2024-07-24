use crate::config::Config;
use crate::consts::{TXN_BATCH_DELAY_INTERVAL, TXN_BATCH_INTERVAL};
use crate::context::TxnCtx;
use crate::get_report_timer;
use crate::protocol::crypto::Hash;
use crate::protocol::transaction::Txn;
use crate::protocol::CryptoScheme;
use crate::stage::pass;
use crate::utils::{CopycatError, NodeId};

use std::collections::{HashSet, VecDeque};
use std::sync::atomic::Ordering;
use std::sync::Arc;

use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::{mpsc, Semaphore};
use tokio::time::{Duration, Instant};

use atomic_float::AtomicF64;
use tokio_metrics::TaskMonitor;

struct TxnValidation {
    txn_seen: HashSet<Hash>,
    crypto_scheme: CryptoScheme,
}

impl TxnValidation {
    pub fn new(crypto_scheme: CryptoScheme) -> Self {
        Self {
            txn_seen: HashSet::new(),
            crypto_scheme,
        }
    }

    pub async fn validate(
        &mut self,
        txn_batch: std::collections::vec_deque::Drain<'_, (u64, Arc<Txn>)>,
        concurrency: usize,
    ) -> Result<Vec<(NodeId, (Arc<Txn>, Arc<TxnCtx>))>, CopycatError> {
        let mut correct_txns = vec![];
        let mut verification_time = 0f64;

        for (src, txn) in txn_batch.into_iter() {
            let txn_ctx = Arc::new(TxnCtx::from_txn(&txn)?);
            let hash = &txn_ctx.id;

            // ignore duplicates
            if self.txn_seen.contains(hash) {
                continue;
            }
            self.txn_seen.insert(*hash);

            // ignore invalid txns
            let (valid, vtime) = txn.validate(self.crypto_scheme)?;
            verification_time += vtime;
            if !valid {
                continue;
            }

            correct_txns.push((src, (txn, txn_ctx)));
        }

        let concurrent_verify_time = verification_time / concurrency as f64;
        // 1ms
        if concurrent_verify_time > 0.001 {
            tokio::time::sleep(Duration::from_secs_f64(concurrent_verify_time)).await;
        }

        Ok(correct_txns)
    }
}

fn get_txn_validation(crypto_scheme: CryptoScheme) -> TxnValidation {
    TxnValidation::new(crypto_scheme)
}

pub async fn txn_validation_thread(
    id: NodeId,
    _config: Config,
    crypto_scheme: CryptoScheme,
    mut req_recv: mpsc::Receiver<Arc<Txn>>,
    mut peer_txn_recv: mpsc::Receiver<(NodeId, Vec<Arc<Txn>>)>,
    validated_txn_send: mpsc::Sender<Vec<(NodeId, (Arc<Txn>, Arc<TxnCtx>))>>,
    concurrency: Arc<Semaphore>,
    monitor: TaskMonitor,
) {
    pf_info!(id; "txn validation stage starting...");

    const VALIDATION_BATCH_SIZE: usize = 100usize;

    let delay: Arc<AtomicF64> = Arc::new(AtomicF64::new(0f64));
    let mut insert_delay_time = Instant::now() + TXN_BATCH_DELAY_INTERVAL;

    let mut txn_validation_stage = get_txn_validation(crypto_scheme);
    let mut txn_buffer = VecDeque::new();
    let mut txn_batch_time = None;

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
                // batch validating txns
                let _permit = match concurrency.acquire().await {
                    Ok(permit) => permit,
                    Err(e) => {
                        pf_error!(id; "failed to acquire allowed concurrency: {:?}", e);
                        continue;
                    }
                };

                let num_txns_to_drain = std::cmp::min(VALIDATION_BATCH_SIZE, txn_buffer.len());
                let txns_to_validate = txn_buffer.drain(0..num_txns_to_drain);

                txn_batches_validated += 1;
                txns_validated += num_txns_to_drain;

                let txns = match txn_validation_stage.validate(txns_to_validate, 1).await {
                    Ok(txns) => txns,
                    Err(e) => {
                        pf_error!(id; "error validating txns: {:?}", e);
                        continue;
                    }
                };

                if txn_buffer.len() == 0 {
                    txn_batch_time = None;
                }

                drop(_permit);

                if let Err(e) = validated_txn_send.send(txns).await {
                    pf_error!(id; "failed to send to validated_txn pipe: {:?}", e);
                }
            }

            _ = pass(), if Instant::now() > insert_delay_time => {
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
