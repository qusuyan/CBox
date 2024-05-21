use crate::context::TxnCtx;
use crate::protocol::crypto::Hash;
use crate::protocol::transaction::Txn;
use crate::protocol::CryptoScheme;
use crate::utils::{CopycatError, NodeId};

use dashmap::DashSet;
use std::sync::Arc;
use tokio::sync::{mpsc, Semaphore};
use tokio::time::{Duration, Instant};

use crate::config::Config;

pub struct TxnValidation {
    txn_seen: DashSet<Hash>,
    crypto_scheme: CryptoScheme,
}

impl TxnValidation {
    pub fn new(crypto_scheme: CryptoScheme) -> Self {
        Self {
            txn_seen: DashSet::new(),
            crypto_scheme,
        }
    }

    pub async fn validate(&self, txn: Arc<Txn>) -> Result<Option<Arc<TxnCtx>>, CopycatError> {
        let txn_ctx = Arc::new(TxnCtx::from_txn(&txn)?);
        let hash = &txn_ctx.id;

        if self.txn_seen.contains(hash) {
            return Ok(Some(txn_ctx));
        }

        if !txn.validate(self.crypto_scheme).await? {
            return Ok(None);
        }

        self.txn_seen.insert(*hash);
        Ok(Some(txn_ctx))
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
    validated_txn_send: mpsc::Sender<(NodeId, (Arc<Txn>, Arc<TxnCtx>))>,
) {
    pf_info!(id; "txn validation stage starting...");

    let txn_validation_stage = Arc::new(get_txn_validation(crypto_scheme));

    let max_concurrency = 2;
    let sem = Arc::new(Semaphore::new(max_concurrency));

    let mut report_time = Instant::now() + Duration::from_secs(60);
    let mut self_txns_validated = 0;
    let mut peer_txns_validated = 0;

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
                self_txns_validated += 1;

                let semaphore = sem.clone();
                let txn_validator = txn_validation_stage.clone();
                let validated_txn_sender = validated_txn_send.clone();
                tokio::task::spawn(async move {
                    let txn_ctx = {
                        let _ = match semaphore.acquire().await {
                            Ok(permit) => permit,
                            Err(e) => {
                                pf_error!(id; "failed to acquire concurrency: {:?}", e);
                                return;
                            }
                        };
                        match txn_validator.validate(txn.clone()).await {
                            Ok(txn_ctx) => {
                                if let Some(ctx) = txn_ctx {
                                    ctx
                                } else {
                                    pf_trace!(id; "got invalid or duplicate txn {:?}, ignoring...", txn);
                                    return;
                                }
                            }
                            Err(e) => {
                                pf_error!(id; "error validating txn: {:?}", e);
                                return;
                            }
                        }
                    };

                    if let Err(e) = validated_txn_sender.send((id, (txn, txn_ctx))).await {
                        pf_error!(id; "failed to send to validated_txn pipe: {:?}", e);
                    }
                });

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
                peer_txns_validated += 1;

                let semaphore = sem.clone();
                let txn_validator = txn_validation_stage.clone();
                let validated_txn_sender = validated_txn_send.clone();
                tokio::task::spawn(async move {
                    let txn_ctx = {
                        let _ = match semaphore.acquire().await {
                            Ok(permit) => permit,
                            Err(e) => {
                                pf_error!(id; "failed to acquire concurrency: {:?}", e);
                                return;
                            }
                        };
                        match txn_validator.validate(txn.clone()).await {
                            Ok(txn_ctx) => {
                                if let Some(ctx) = txn_ctx {
                                    ctx
                                } else {
                                    pf_trace!(id; "got invalid or duplicate txn {:?}, ignoring...", txn);
                                    return;
                                }
                            }
                            Err(e) => {
                                pf_error!(id; "error validating txn: {:?}", e);
                                return;
                            }
                        }
                    };

                    if let Err(e) = validated_txn_sender.send((id, (txn, txn_ctx))).await {
                        pf_error!(id; "failed to send to validated_txn pipe: {:?}", e);
                    }
                });
            }
            _ = tokio::time::sleep_until(report_time) => {
                pf_info!(id; "In the last minute: self_txns_validated: {}, peer_txns_validated: {}", self_txns_validated, peer_txns_validated);
                self_txns_validated = 0;
                peer_txns_validated = 0;
                report_time = Instant::now() + Duration::from_secs(60);
            }
        };
    }
}
