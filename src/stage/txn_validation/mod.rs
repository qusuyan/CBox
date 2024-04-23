mod dummy;
use dummy::DummyTxnValidation;

mod bitcoin;
use bitcoin::BitcoinTxnValidation;

mod avalanche;
use avalanche::AvalancheTxnValidation;

use crate::context::TxnCtx;
use crate::protocol::transaction::Txn;
use crate::protocol::CryptoScheme;
use crate::utils::{CopycatError, NodeId};

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{Duration, Instant};

use crate::config::Config;

#[async_trait]
pub trait TxnValidation: Send + Sync {
    async fn validate(&mut self, txn: Arc<Txn>) -> Result<Option<Arc<TxnCtx>>, CopycatError>;
}

fn get_txn_validation(config: Config, crypto_scheme: CryptoScheme) -> Box<dyn TxnValidation> {
    match config {
        Config::Dummy => Box::new(DummyTxnValidation::new()),
        Config::Bitcoin { .. } => Box::new(BitcoinTxnValidation::new(crypto_scheme)),
        Config::Avalanche { .. } => Box::new(AvalancheTxnValidation::new(crypto_scheme)),
    }
}

pub async fn txn_validation_thread(
    id: NodeId,
    config: Config,
    crypto_scheme: CryptoScheme,
    mut req_recv: mpsc::Receiver<Arc<Txn>>,
    mut peer_txn_recv: mpsc::Receiver<(NodeId, Arc<Txn>)>,
    validated_txn_send: mpsc::Sender<(NodeId, (Arc<Txn>, Arc<TxnCtx>))>,
) {
    pf_info!(id; "txn validation stage starting...");

    let mut txn_validation_stage = get_txn_validation(config, crypto_scheme);

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

                match txn_validation_stage.validate(txn.clone()).await {
                    Ok(txn_ctx) => {
                        if let Some(ctx) = txn_ctx {
                            if let Err(e) = validated_txn_send.send((id, (txn, ctx))).await {
                                pf_error!(id; "failed to send to validated_txn pipe: {:?}", e);
                            }
                        } else {
                            pf_trace!(id; "got invalid or duplicate txn {:?}, ignoring...", txn);
                        }
                    }
                    Err(e) => {
                        pf_error!(id; "error validating txn: {:?}", e);
                    }
                }
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

                match txn_validation_stage.validate(txn.clone()).await {
                    Ok(txn_ctx) => {
                        if let Some(ctx) = txn_ctx {
                            if let Err(e) = validated_txn_send.send((src, (txn, ctx))).await {
                                pf_error!(id; "failed to send to validated_txn pipe: {:?}", e);
                            }
                        } else {
                            pf_trace!(id; "got invalid or duplicate txn {:?}, ignoring...", txn);
                        }
                    }
                    Err(e) => {
                        pf_error!(id; "error validating txn: {:?}", e);
                    }
                }
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
