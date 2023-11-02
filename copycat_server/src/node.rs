use std::fmt::Debug;
use std::sync::Arc;

use copycat_utils::{CopycatError, NodeId};

use serde::{de::DeserializeOwned, Serialize};
use tokio::{sync::mpsc, task::JoinHandle};

use crate::composition::SystemCompose;
use crate::peers::PeerMessenger;
use crate::stage::txn_dissemination::txn_dissemination_thread;
use crate::stage::txn_validation::txn_validation_thread;

pub struct Node<TxnType, BlockType> {
    req_send: mpsc::Sender<TxnType>,
    peer_messenger: Arc<PeerMessenger<TxnType, BlockType>>,
    _txn_validation_handle: JoinHandle<()>,
}

impl<TxnType, BlockType> Node<TxnType, BlockType>
where
    TxnType: 'static + Debug + Clone + Serialize + DeserializeOwned + Sync + Send,
    BlockType: 'static + Debug + Serialize + DeserializeOwned + Sync + Send,
{
    pub async fn init(id: NodeId, compose: SystemCompose) -> Result<Self, CopycatError> {
        let (
            peer_messenger,
            mut peer_txn_recv,
            mut peer_blk_recv,
            mut peer_consensus_recv,
            mut peer_pmaker_recv,
        ) = PeerMessenger::new(id).await?;

        let peer_messenger = Arc::new(peer_messenger);

        let (req_send, req_recv) = mpsc::channel(1024 * 1024);
        let (validated_txn_send, validated_txn_recv) = mpsc::channel(1024 * 1024);
        let (txn_ready_send, txn_ready_recv) = mpsc::channel(1024 * 1024);

        let _txn_validation_handle = tokio::spawn(txn_validation_thread(
            id,
            compose.txn_validation_stage,
            req_recv,
            peer_txn_recv,
            validated_txn_send,
        ));

        let _txn_dissemination_handle = tokio::spawn(txn_dissemination_thread(
            id,
            compose.txn_dissemination_stage,
            peer_messenger.clone(),
            validated_txn_recv,
            txn_ready_send,
        ));

        Ok(Self {
            req_send,
            peer_messenger,
            _txn_validation_handle,
        })
    }
}
