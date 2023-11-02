mod dummy;
use dummy::DummyTxnValidation;

use copycat_utils::NodeId;

use serde::{de::DeserializeOwned, Serialize};
use tokio::sync::mpsc;

pub trait TxnValidation<TxnType>: Send + Sync {
    fn validate(&self, txn: &TxnType) -> bool;
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
    mut req_recv: mpsc::Receiver<TxnType>,
    mut peer_txn_recv: mpsc::UnboundedReceiver<(NodeId, TxnType)>,
    validated_txn_send: mpsc::Sender<(TxnType, bool)>,
) where
    TxnType: 'static + std::fmt::Debug + Serialize + DeserializeOwned + Sync + Send,
{
    let txn_validation_stage = get_txn_validation::<TxnType>(id, txn_validation_type);

    loop {
        let (txn, should_disseminate) = tokio::select! {
            new_txn = req_recv.recv() => {
                match new_txn {
                    Some(txn) => (txn, true),
                    None => {
                        log::error!("Node {id}: request channel closed");
                        continue;
                    }
                }
            },
            new_peer_txn = peer_txn_recv.recv() => {
                match new_peer_txn {
                    Some((_src, txn)) => (txn, false),
                    None => {
                        log::error!("Node {id}: request channel closed");
                        continue;
                    }
                }
            }
        };

        if txn_validation_stage.validate(&txn) {
            if let Err(e) = validated_txn_send.send((txn, should_disseminate)).await {
                log::error!(
                    "Node {id}: failed to send from txn_validation to txn_dissemination: {e:?}"
                );
            }
        } else {
            log::warn!("Node {id}: got invalid txn, ignoring...");
        }
    }
}
