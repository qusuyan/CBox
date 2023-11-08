mod broadcast;
use broadcast::BroadcastTxnDissemination;

use async_trait::async_trait;
use copycat_utils::{CopycatError, NodeId};

use serde::{de::DeserializeOwned, Serialize};
use tokio::sync::mpsc;

use std::sync::Arc;

use crate::{block::ChainType, peers::PeerMessenger};

#[async_trait]
pub trait TxnDissemination<TxnType>: Send + Sync {
    async fn disseminate(&self, txn: &TxnType) -> Result<(), CopycatError>;
}

fn get_txn_dissemination<TxnType, BlockType>(
    chain_type: ChainType,
    peer_messenger: Arc<PeerMessenger<TxnType, BlockType>>,
) -> Box<dyn TxnDissemination<TxnType>>
where
    TxnType: 'static + std::fmt::Debug + Clone + Serialize + DeserializeOwned + Sync + Send,
    BlockType: 'static + std::fmt::Debug + Serialize + DeserializeOwned + Sync + Send,
{
    match chain_type {
        ChainType::Dummy => Box::new(BroadcastTxnDissemination::new(peer_messenger)),
        ChainType::Bitcoin => todo!(),
    }
}

pub async fn txn_dissemination_thread<TxnType, BlockType>(
    id: NodeId,
    chain_type: ChainType,
    peer_messenger: Arc<PeerMessenger<TxnType, BlockType>>,
    mut validated_txn_recv: mpsc::Receiver<(Arc<TxnType>, bool)>,
    txn_ready_send: mpsc::Sender<Arc<TxnType>>,
) where
    TxnType: 'static + Clone + std::fmt::Debug + Serialize + DeserializeOwned + Sync + Send,
    BlockType: 'static + std::fmt::Debug + Serialize + DeserializeOwned + Sync + Send,
{
    let txn_dissemination_stage = get_txn_dissemination(chain_type, peer_messenger);

    loop {
        let (txn, should_disseminate) = match validated_txn_recv.recv().await {
            Some(txn) => txn,
            None => {
                log::error!("Node {id}: validated_txn pipe closed unexpectedly");
                continue;
            }
        };

        if should_disseminate {
            if let Err(e) = txn_dissemination_stage.disseminate(&txn).await {
                log::error!("Node {id}: failed to disseminate txn: {e:?}");
                continue;
            }
        }

        if let Err(e) = txn_ready_send.send(txn).await {
            log::error!("Node {id}: failed to send to txn_ready pipe: {e:?}");
            continue;
        }
    }
}
