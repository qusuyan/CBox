use crate::config::Config;
use crate::context::TxnCtx;
use crate::protocol::crypto::Hash;
use crate::protocol::transaction::Txn;
use crate::protocol::CryptoScheme;
use crate::utils::{CopycatError, NodeId};

use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{Duration, Instant};

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
    mut peer_txn_recv: mpsc::Receiver<(NodeId, Arc<Txn>)>,
    validated_txn_send: mpsc::Sender<Vec<(NodeId, (Arc<Txn>, Arc<TxnCtx>))>>,
) {
    pf_info!(id; "txn validation stage starting...");

    let txn_batch_interval = Duration::from_millis(100);
    let mut txn_validation_stage = get_txn_validation(crypto_scheme);
    let mut txn_buffer = vec![];
    let mut txn_batch_time = Instant::now() + txn_batch_interval;

    async fn wait_validation_batch(batch: &Vec<(u64, Arc<Txn>)>, timeout: Instant) {
        let notify = tokio::sync::Notify::new();
        if batch.len() == 0 {
            notify.notified().await;
        } else if batch.len() > 100 {
            return;
        }
        tokio::time::sleep_until(timeout).await;
    }

    let mut report_time = Instant::now() + Duration::from_secs(60);
    let mut self_txns_recved = 0;
    let mut peer_txns_recved = 0;

    loop {
        tokio::select! {
            new_txn = req_recv.recv() => {
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
            },
            new_peer_txn = peer_txn_recv.recv() => {
                let (src, txn) = match new_peer_txn {
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

                pf_trace!(id; "got from peer {} new txn {:?}", src, txn);
                peer_txns_recved += 1;
                txn_buffer.push((src, txn));
            }
            _ = wait_validation_batch(&txn_buffer, txn_batch_time) => {
                let txns_to_validate = txn_buffer.drain(0..).collect();
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
                txn_batch_time += txn_batch_interval;
            }
            _ = tokio::time::sleep_until(report_time) => {
                pf_info!(id; "In the last minute: self_txns_recved: {}, peer_txns_recved: {}", self_txns_recved, peer_txns_recved);
                self_txns_recved = 0;
                peer_txns_recved = 0;
                report_time = Instant::now() + Duration::from_secs(60);
            }
        };
    }
}
