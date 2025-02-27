use super::{BlockManagement, CurBlockState};
use crate::config::DiemConfig;
use crate::context::{BlkCtx, TxnCtx};
use crate::peers::PeerMessenger;
use crate::protocol::block::Block;
use crate::transaction::Txn;
use crate::{CopycatError, NodeId};

use atomic_float::AtomicF64;
use std::sync::Arc;

use async_trait::async_trait;

pub struct DiemBlockManagement {
    id: NodeId,
    blk_size: usize,
    delay: Arc<AtomicF64>,
    peer_messenger: Arc<PeerMessenger>,
}

impl DiemBlockManagement {
    pub fn new(
        id: NodeId,
        config: DiemConfig,
        delay: Arc<AtomicF64>,
        peer_messenger: Arc<PeerMessenger>,
    ) -> Self {
        Self {
            id,
            blk_size: config.blk_size,
            delay,
            peer_messenger,
        }
    }
}

#[async_trait]
impl BlockManagement for DiemBlockManagement {
    async fn record_new_txn(
        &mut self,
        txn: Arc<Txn>,
        ctx: Arc<TxnCtx>,
    ) -> Result<bool, CopycatError> {
        todo!()
    }

    async fn prepare_new_block(&mut self) -> Result<CurBlockState, CopycatError> {
        todo!()
    }

    async fn wait_to_propose(&self) -> Result<(), CopycatError> {
        todo!()
    }

    async fn get_new_block(&mut self) -> Result<(Arc<Block>, Arc<BlkCtx>), CopycatError> {
        todo!()
    }

    async fn validate_block(
        &mut self,
        block: Arc<Block>,
        ctx: Arc<BlkCtx>,
    ) -> Result<Vec<(Arc<Block>, Arc<BlkCtx>)>, CopycatError> {
        todo!()
    }

    async fn handle_pmaker_msg(&mut self, msg: Vec<u8>) -> Result<(), CopycatError> {
        todo!()
    }

    async fn handle_peer_blk_req(
        &mut self,
        peer: NodeId,
        msg: Vec<u8>,
    ) -> Result<(), CopycatError> {
        todo!()
    }

    fn report(&mut self) {}
}
