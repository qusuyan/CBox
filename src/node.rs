use std::collections::HashSet;
use std::sync::Arc;

use crate::protocol::transaction::Txn;
use crate::protocol::{ChainType, CryptoScheme, DissemPattern};
use crate::utils::{CopycatError, NodeId};

use tokio::{sync::mpsc, task::JoinHandle};

use crate::config::Config;
use crate::peers::PeerMessenger;
use crate::stage::commit::commit_thread;
use crate::stage::consensus::block_dissemination::block_dissemination_thread;
use crate::stage::consensus::block_management::block_management_thread;
use crate::stage::consensus::decide::decision_thread;
use crate::stage::pacemaker::pacemaker_thread;
use crate::stage::txn_dissemination::txn_dissemination_thread;
use crate::stage::txn_validation::txn_validation_thread;
// use crate::state::ChainState;

pub struct Node {
    req_send: mpsc::Sender<Arc<Txn>>,
    // _state: Arc<ChainState>,
    // actor threads
    _peer_messenger: Arc<PeerMessenger>,
    _txn_validation_handle: JoinHandle<()>,
    _txn_dissemination_handle: JoinHandle<()>,
    _pacemaker_handle: JoinHandle<()>,
    _block_management_handle: JoinHandle<()>,
    _block_dissemination_handle: JoinHandle<()>,
    _decision_handle: JoinHandle<()>,
    _commit_handle: JoinHandle<()>,
}

impl Node {
    pub async fn init(
        id: NodeId,
        chain_type: ChainType,
        dissem_pattern: DissemPattern,
        crypto_scheme: CryptoScheme,
        config: Config,
        dissem_txns: bool,
        neighbors: HashSet<NodeId>,
    ) -> Result<(Self, mpsc::Receiver<Arc<Txn>>), CopycatError> {
        pf_trace!(id; "starting: {:?}", chain_type);

        // let state = Arc::new(ChainState::new(chain_type));

        let (
            peer_messenger,
            peer_txn_recv,
            peer_blk_recv,
            peer_consensus_recv,
            peer_pmaker_recv,
            peer_blk_req_recv,
            // peer_blk_resp_recv,
        ) = PeerMessenger::new(id, neighbors).await?;

        let peer_messenger = Arc::new(peer_messenger);

        let (req_send, req_recv) = mpsc::channel::<Arc<Txn>>(0x1000000);
        let (validated_txn_send, validated_txn_recv) = mpsc::channel(0x1000000);
        let (txn_ready_send, txn_ready_recv) = mpsc::channel(0x1000000);
        let (pacemaker_send, pacemaker_recv) = mpsc::channel(0x1000000);
        let (new_block_send, new_block_recv) = mpsc::channel(0x1000000);
        let (block_ready_send, block_ready_recv) = mpsc::channel(0x1000000);
        let (commit_send, commit_recv) = mpsc::channel(0x1000000);
        let (executed_send, executed_recv) = mpsc::channel(0x1000000);

        let _txn_validation_handle = tokio::spawn(txn_validation_thread(
            id,
            config.clone(),
            crypto_scheme,
            req_recv,
            peer_txn_recv,
            validated_txn_send,
        ));

        let _txn_dissemination_handle = tokio::spawn(txn_dissemination_thread(
            id,
            dissem_pattern,
            config.clone(),
            dissem_txns,
            peer_messenger.clone(),
            validated_txn_recv,
            txn_ready_send,
        ));

        let _pacemaker_handle = tokio::spawn(pacemaker_thread(
            id,
            config.clone(),
            peer_messenger.clone(),
            peer_pmaker_recv,
            pacemaker_send,
        ));

        let _block_management_handle = tokio::spawn(block_management_thread(
            id,
            config.clone(),
            crypto_scheme,
            peer_blk_recv,
            peer_blk_req_recv,
            // peer_blk_resp_recv,
            peer_messenger.clone(),
            txn_ready_recv,
            pacemaker_recv,
            new_block_send,
        ));

        let _block_dissemination_handle = tokio::spawn(block_dissemination_thread(
            id,
            dissem_pattern,
            config.clone(),
            peer_messenger.clone(),
            new_block_recv,
            block_ready_send,
        ));

        let _decision_handle = tokio::spawn(decision_thread(
            id,
            config.clone(),
            peer_messenger.clone(),
            peer_consensus_recv,
            block_ready_recv,
            commit_send,
        ));

        let _commit_handle = tokio::spawn(commit_thread(
            id,
            config.clone(),
            commit_recv,
            executed_send,
        ));

        pf_info!(id; "stages started");

        Ok((
            Self {
                req_send,
                _peer_messenger: peer_messenger,
                _txn_validation_handle,
                _txn_dissemination_handle,
                _pacemaker_handle,
                _block_management_handle,
                _block_dissemination_handle,
                _decision_handle,
                _commit_handle,
            },
            executed_recv,
        ))
    }

    // pub async fn wait_completion(self) -> Result<(), CopycatError> {
    //     self._txn_validation_handle.await?;
    //     self._txn_dissemination_handle.await?;
    //     self._pacemaker_handle.await?;
    //     self._block_management_handle.await?;
    //     self._block_dissemination_handle.await?;
    //     self._decision_handle.await?;
    //     self._commit_handle.await?;
    //     Ok(())
    // }

    pub async fn send_req(&self, txn: Arc<Txn>) -> Result<(), CopycatError> {
        if let Err(e) = self.req_send.send(txn).await {
            return Err(CopycatError(format!("{e:?}")));
        }
        Ok(())
    }
}
