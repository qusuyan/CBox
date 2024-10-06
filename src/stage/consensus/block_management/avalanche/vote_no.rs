use super::{BlockManagement, CurBlockState};
use crate::config::AvalancheVoteNoConfig;
use crate::context::{BlkCtx, TxnCtx};
use crate::protocol::block::Block;
use crate::protocol::transaction::Txn;
use crate::utils::{CopycatError, NodeId};

use async_trait::async_trait;

use tokio::sync::Notify;

use std::sync::Arc;

pub struct AvalancheVoteNoBlockManagement {
    _id: NodeId,
    _notify: Notify,
}

impl AvalancheVoteNoBlockManagement {
    pub fn new(id: NodeId, _config: AvalancheVoteNoConfig) -> Self {
        Self {
            _id: id,
            _notify: Notify::new(),
        }
    }
}

#[async_trait]
impl BlockManagement for AvalancheVoteNoBlockManagement {
    async fn record_new_txn(
        &mut self,
        _txn: Arc<Txn>,
        _ctx: Arc<TxnCtx>,
    ) -> Result<bool, CopycatError> {
        Ok(false)
    }

    async fn prepare_new_block(&mut self) -> Result<CurBlockState, CopycatError> {
        Ok(CurBlockState::Full)
    }

    async fn wait_to_propose(&self) -> Result<(), CopycatError> {
        self._notify.notified().await;
        Ok(())
    }

    async fn get_new_block(&mut self) -> Result<(Arc<Block>, Arc<BlkCtx>), CopycatError> {
        unreachable!()
    }

    async fn validate_block(
        &mut self,
        block: Arc<Block>,
        ctx: Arc<BlkCtx>,
    ) -> Result<Vec<(Arc<Block>, Arc<BlkCtx>)>, CopycatError> {
        Ok(vec![(block, ctx)]) // send the block to decide phase for voting
    }

    async fn handle_pmaker_msg(&mut self, _msg: Vec<u8>) -> Result<(), CopycatError> {
        unreachable!()
    }

    async fn handle_peer_blk_req(
        &mut self,
        _peer: NodeId,
        _msg: Vec<u8>,
    ) -> Result<(), CopycatError> {
        Ok(()) // ignore the request
    }

    fn report(&mut self) {}
}
