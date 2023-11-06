mod dummy;
use std::sync::Arc;

use dummy::DummyTxnValidation;

use copycat_utils::{CopycatError, NodeId};

use async_trait::async_trait;

use serde::{de::DeserializeOwned, Serialize};
use tokio::sync::mpsc;

#[async_trait]
pub trait TxnValidation<TxnType>: Send + Sync {
    async fn validate(&self, txn: &TxnType) -> Result<bool, CopycatError>;
}

pub enum TxnValidationType {
    Dummy,
}

fn get_txn_validation<TxnType>(
    id: NodeId,
    txn_validation_type: TxnValidationType,
) -> Box<dyn TxnValidation<TxnType>> {
    match txn_validation_type {
        TxnValidationType::Dummy => Box::new(DummyTxnValidation::new(id)),
    }
}

pub async fn txn_validation_thread<TxnType>(
    id: NodeId,
    txn_validation_type: TxnValidationType,
    mut req_recv: mpsc::Receiver<Arc<TxnType>>,
    mut peer_txn_recv: mpsc::UnboundedReceiver<(NodeId, Arc<TxnType>)>,
    validated_txn_send: mpsc::Sender<(Arc<TxnType>, bool)>,
) where
    TxnType: 'static + std::fmt::Debug + Serialize + DeserializeOwned + Sync + Send,
{
    let txn_validation_stage = get_txn_validation::<TxnType>(id, txn_validation_type);

    loop {
        let (src, txn) = tokio::select! {
            new_txn = req_recv.recv() => {
                match new_txn {
                    Some(txn) => (id, txn),
                    None => {
                        log::error!("Node {id}: request pipe closed");
                        continue;
                    }
                }
            },
            new_peer_txn = peer_txn_recv.recv() => {
                match new_peer_txn {
                    Some(txn) => txn,
                    None => {
                        log::error!("Node {id}: peer_txn pipe closed");
                        continue;
                    }
                }
            }
        };

        match txn_validation_stage.validate(&txn).await {
            Ok(valid) => {
                if !valid {
                    log::warn!("Node {id}: got invalid txn, ignoring...");
                    continue;
                }

                let should_disseminate = src == id;
                if let Err(e) = validated_txn_send.send((txn, should_disseminate)).await {
                    log::error!("Node {id}: failed to send to validated_txn pipe: {e:?}");
                    continue;
                }
            }
            Err(e) => {
                log::error!("Node {id}: error validating txn: {e:?}");
                continue;
            }
        }
    }
}
