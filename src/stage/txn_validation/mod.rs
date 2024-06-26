use crate::config::Config;
use crate::context::TxnCtx;
use crate::protocol::crypto::Hash;
use crate::protocol::transaction::Txn;
use crate::protocol::CryptoScheme;
use crate::utils::{CopycatError, NodeId};

use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::{mpsc, Semaphore};
use tokio::time::{Duration, Instant};

use atomic_float::AtomicF64;
use std::sync::atomic::Ordering;
use tokio_metrics::TaskMonitor;

pub struct TxnValidation {
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
        txn_batch: Vec<(NodeId, Arc<Txn>)>,
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

        // 1ms
        if verification_time > 0.001 {
            tokio::time::sleep(Duration::from_secs_f64(verification_time)).await;
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

    let validation_batch_size = 100usize;

    let delay: Arc<AtomicF64> = Arc::new(AtomicF64::new(0f64));
    let insert_delay_interval = Duration::from_millis(50);
    let mut insert_delay_time = Instant::now() + insert_delay_interval;

    let txn_batch_interval = Duration::from_millis(100);
    let mut txn_validation_stage = get_txn_validation(crypto_scheme);
    let mut txn_buffer = vec![];
    let mut txn_batch_time = None;

    async fn wait_validation_batch(
        batch: &Vec<(u64, Arc<Txn>)>,
        max_batch_size: usize,
        timeout: Option<Instant>,
    ) {
        let notify = tokio::sync::Notify::new();
        if batch.len() == 0 || timeout.is_none() {
            notify.notified().await;
        } else if batch.len() > max_batch_size {
            return;
        }
        tokio::time::sleep_until(timeout.unwrap()).await;
    }

    let mut report_time = Instant::now() + Duration::from_secs(60);
    let mut self_txns_recved = 0;
    let mut peer_txns_recved = 0;
    let mut txn_batches_validated = 0;
    let mut txns_validated = 0;

    let mut intervals = monitor.intervals();

    loop {
        tokio::select! {
            new_txn = req_recv.recv() => {
                let _ = match concurrency.acquire().await {
                    Ok(permit) => permit,
                    Err(e) => {
                        pf_error!(id; "failed to acquire allowed concurrency: {:?}", e);
                        continue;
                    }
                };

                let txn = match new_txn {
                    Some(txn) => txn,
                    None => {
                        pf_error!(id; "request pipe closed");
                        continue;
                    }
                };

                pf_trace!(id; "got from self new txn {:?}", txn);
                self_txns_recved += 1;
                txn_buffer.push((id, txn));

                if txn_batch_time.is_none() {
                    txn_batch_time = Some(Instant::now() + txn_batch_interval);
                }
            },
            new_peer_txn = peer_txn_recv.recv() => {
                let _ = match concurrency.acquire().await {
                    Ok(permit) => permit,
                    Err(e) => {
                        pf_error!(id; "failed to acquire allowed concurrency: {:?}", e);
                        continue;
                    }
                };

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
            }
            _ = wait_validation_batch(&txn_buffer, validation_batch_size, txn_batch_time), if txn_batch_time.is_some() => {
                let _ = match concurrency.acquire().await {
                    Ok(permit) => permit,
                    Err(e) => {
                        pf_error!(id; "failed to acquire allowed concurrency: {:?}", e);
                        continue;
                    }
                };

                let num_txns_to_drain = std::cmp::min(validation_batch_size, txn_buffer.len());
                let txns_to_validate = txn_buffer.drain(0..num_txns_to_drain).collect();

                txn_batches_validated += 1;
                txns_validated += num_txns_to_drain;

                match txn_validation_stage.validate(txns_to_validate).await {
                    Ok(txns) => {
                        if let Err(e) = validated_txn_send.send(txns).await {
                            pf_error!(id; "failed to send to validated_txn pipe: {:?}", e);
                        }
                    },
                    Err(e) => {
                        pf_error!(id; "error validating txns: {:?}", e);
                    }
                };
                txn_batch_time = None;
            }
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
                } else {
                    tokio::task::yield_now().await;
                }
                insert_delay_time = Instant::now() + insert_delay_interval;
            }
            _ = tokio::time::sleep_until(report_time) => {
                // report basic statistics
                pf_info!(id; "In the last minute: self_txns_recved: {}, peer_txns_recved: {}", self_txns_recved, peer_txns_recved);
                pf_info!(id; "In the last minute: txn_batches_validated: {}, txns_validated: {}", txn_batches_validated, txns_validated);

                let metrics = intervals.next().unwrap();
                let avg_sched_duration = metrics.mean_scheduled_duration().as_secs_f64();
                let poll_duration = metrics.total_poll_duration.as_secs_f64();
                pf_info!(id; "In the last minute: avg_sched_duration: {}, total_poll_duration: {}", avg_sched_duration, poll_duration);


                self_txns_recved = 0;
                peer_txns_recved = 0;
                txn_batches_validated = 0;
                txns_validated = 0;
                // reset report time
                report_time = Instant::now() + Duration::from_secs(60);
            }
        }
    }
}
