mod broadcast;
use broadcast::BroadcastTxnDissemination;

use async_trait::async_trait;
use copycat_utils::{CopycatError, NodeId};

use serde::{de::DeserializeOwned, Serialize};
use tokio::sync::mpsc;

use std::sync::Arc;

use crate::peers::PeerMessenger;

pub enum TxnDisseminationType {
    Broadcast,
}

#[async_trait]
pub trait TxnDissemination<TxnType>: Send + Sync {
    async fn disseminate(&self, txn: TxnType) -> Result<(), CopycatError>;
}

fn get_txn_dissemination<TxnType, BlockType>(
    id: NodeId,
    txn_dissemination_type: TxnDisseminationType,
    peer_messenger: Arc<PeerMessenger<TxnType, BlockType>>,
) -> Box<dyn TxnDissemination<TxnType>>
where
    TxnType: 'static + std::fmt::Debug + Serialize + DeserializeOwned + Sync + Send,
    BlockType: 'static + std::fmt::Debug + Serialize + DeserializeOwned + Sync + Send,
{
    match txn_dissemination_type {
        TxnDisseminationType::Broadcast => Box::new(BroadcastTxnDissemination::new(peer_messenger)),
    }
}

pub async fn txn_dissemination_thread<BlockType, TxnType>(
    id: NodeId,
    txn_dissemination_type: TxnDisseminationType,
    peer_messenger: Arc<PeerMessenger<TxnType, BlockType>>,
    mut validated_txn_recv: mpsc::Receiver<(TxnType, bool)>,
    txn_ready_send: mpsc::Sender<TxnType>,
) where
    TxnType: 'static + Clone + std::fmt::Debug + Serialize + DeserializeOwned + Sync + Send,
    BlockType: 'static + std::fmt::Debug + Serialize + DeserializeOwned + Sync + Send,
{
    let txn_dissemination_stage = get_txn_dissemination(id, txn_dissemination_type, peer_messenger);

    loop {
        let (txn, should_disseminate) = match validated_txn_recv.recv().await {
            Some(txn) => txn,
            None => {
                log::error!("Node {id}: getting next valid txn failed");
                continue;
            }
        };

        if should_disseminate {
            if let Err(e) = txn_dissemination_stage.disseminate(txn.clone()).await {
                log::error!("Node {id}: failed to disseminate txn: {e:?}");
            }
        }

        txn_ready_send.send(txn);
    }
}
