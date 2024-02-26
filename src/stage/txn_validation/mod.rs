mod dummy;
use dummy::DummyTxnValidation;

mod bitcoin;
use bitcoin::BitcoinTxnValidation;

use crate::protocol::transaction::Txn;
use crate::protocol::CryptoScheme;
use crate::utils::{CopycatError, NodeId};

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::mpsc;

use crate::config::Config;

#[async_trait]
pub trait TxnValidation: Send + Sync {
    async fn validate(&mut self, txn: Arc<Txn>) -> Result<bool, CopycatError>;
}

fn get_txn_validation(config: Config, crypto_scheme: CryptoScheme) -> Box<dyn TxnValidation> {
    match config {
        Config::Dummy => Box::new(DummyTxnValidation::new()),
        Config::Bitcoin { .. } => Box::new(BitcoinTxnValidation::new(crypto_scheme)),
    }
}

pub async fn txn_validation_thread(
    id: NodeId,
    config: Config,
    crypto_scheme: CryptoScheme,
    mut req_recv: mpsc::Receiver<Arc<Txn>>,
    mut peer_txn_recv: mpsc::Receiver<(NodeId, Arc<Txn>)>,
    validated_txn_send: mpsc::Sender<(NodeId, Arc<Txn>)>,
) {
    log::info!("txn validation stage starting...");

    let mut txn_validation_stage = get_txn_validation(config, crypto_scheme);

    loop {
        let (src, txn) = tokio::select! {
            new_txn = req_recv.recv() => {
                match new_txn {
                    Some(txn) => (id, txn),
                    None => {
                        log::error!("request pipe closed");
                        continue;
                    }
                }
            },
            new_peer_txn = peer_txn_recv.recv() => {
                match new_peer_txn {
                    Some((src, txn)) => {
                        if src == id {
                            // ignore txns sent by self
                            continue;
                        }
                        (src, txn)
                    },
                    None => {
                        log::error!("peer_txn pipe closed");
                        continue;
                    },
                }
            }
        };

        log::trace!("got from {src} new txn {txn:?}");

        match txn_validation_stage.validate(txn.clone()).await {
            Ok(valid) => {
                if !valid {
                    log::trace!("got invalid txn {txn:?}, ignoring...");
                }

                if let Err(e) = validated_txn_send.send((src, txn)).await {
                    log::error!("failed to send to validated_txn pipe: {e:?}");
                }
            }
            Err(e) => {
                log::error!("error validating txn: {e:?}");
            }
        }
    }
}
