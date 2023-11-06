use std::fmt::Debug;
use std::sync::Arc;

use copycat_utils::{CopycatError, NodeId};

use serde::{de::DeserializeOwned, Serialize};
use tokio::{sync::mpsc, task::JoinHandle};

use crate::chain::BlockTrait;
use crate::composition::SystemCompose;
use crate::peers::PeerMessenger;
use crate::stage::commit::commit_thread;
use crate::stage::consensus::block_creation::block_creation_thread;
use crate::stage::consensus::block_dissemination::block_dissemination_thread;
use crate::stage::consensus::block_validation::block_validation_thread;
use crate::stage::consensus::decide::decision_thread;
use crate::stage::pacemaker::pacemaker_thread;
use crate::stage::txn_dissemination::txn_dissemination_thread;
use crate::stage::txn_validation::txn_validation_thread;

pub struct Node<TxnType, BlockType> {
    req_send: mpsc::Sender<Arc<TxnType>>,
    executed_recv: mpsc::Receiver<Arc<TxnType>>,
    _peer_messenger: Arc<PeerMessenger<TxnType, BlockType>>,
    // actor threads
    _txn_validation_handle: JoinHandle<()>,
    _txn_dissemination_handle: JoinHandle<()>,
    _pacemaker_handle: JoinHandle<()>,
    _block_creation_handle: JoinHandle<()>,
    _block_dissemination_handle: JoinHandle<()>,
    _block_validation_handle: JoinHandle<()>,
    _decision_handle: JoinHandle<()>,
}

impl<TxnType, BlockType> Node<TxnType, BlockType>
where
    TxnType: 'static + Debug + Clone + Serialize + DeserializeOwned + Sync + Send,
    BlockType:
        'static + std::fmt::Debug + Clone + Serialize + DeserializeOwned + BlockTrait<TxnType>,
{
    pub async fn init(id: NodeId, compose: SystemCompose) -> Result<Self, CopycatError> {
        let (peer_messenger, peer_txn_recv, peer_blk_recv, peer_consensus_recv, peer_pmaker_recv) =
            PeerMessenger::new(id).await?;

        let peer_messenger = Arc::new(peer_messenger);

        let (req_send, req_recv) = mpsc::channel::<Arc<TxnType>>(0x1000000);
        let (validated_txn_send, validated_txn_recv) = mpsc::channel(0x1000000);
        let (txn_ready_send, txn_ready_recv) = mpsc::channel(0x1000000);
        let (should_propose_send, should_propose_recv) = mpsc::channel(0x1000000);
        let (new_block_send, new_block_recv) = mpsc::channel(0x1000000);
        let (block_ready_send, block_ready_recv) = mpsc::channel(0x1000000);
        let (commit_send, commit_recv) = mpsc::channel(0x1000000);
        let (executed_send, executed_recv) = mpsc::channel(0x1000000);

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

        let _pacemaker_handle = tokio::spawn(pacemaker_thread(
            id,
            compose.pacemaker_stage,
            peer_messenger.clone(),
            peer_pmaker_recv,
            should_propose_send,
        ));

        let _block_creation_handle = tokio::spawn(block_creation_thread(
            id,
            compose.block_creation_stage,
            txn_ready_recv,
            should_propose_recv,
            new_block_send,
        ));

        let _block_dissemination_handle = tokio::spawn(block_dissemination_thread(
            id,
            compose.block_dissemination_stage,
            peer_messenger.clone(),
            new_block_recv,
            block_ready_send.clone(),
        ));

        let _block_validation_handle = tokio::spawn(block_validation_thread(
            id,
            compose.block_validation_stage,
            peer_blk_recv,
            block_ready_send,
        ));

        let _decision_handle = tokio::spawn(decision_thread(
            id,
            compose.decision_stage,
            peer_messenger.clone(),
            peer_consensus_recv,
            block_ready_recv,
            commit_send,
        ));

        let _commit_handle = tokio::spawn(commit_thread(
            id,
            compose.commit_type,
            commit_recv,
            executed_send,
        ));

        Ok(Self {
            req_send,
            executed_recv,
            _peer_messenger: peer_messenger,
            _txn_validation_handle,
            _txn_dissemination_handle,
            _pacemaker_handle,
            _block_creation_handle,
            _block_dissemination_handle,
            _block_validation_handle,
            _decision_handle,
        })
    }
}
