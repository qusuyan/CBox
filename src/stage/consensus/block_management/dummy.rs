use super::BlockManagement;

use crate::context::{BlkCtx, TxnCtx};
use crate::protocol::block::{Block, BlockHeader};
use crate::protocol::transaction::Txn;
use crate::utils::{CopycatError, NodeId};

use async_trait::async_trait;
use get_size::GetSize;

use std::sync::Arc;
use tokio::time::{Duration, Instant};

pub struct DummyBlockManagement {
    mem_pool: Vec<Arc<Txn>>,
    last_block: Instant,
}

impl DummyBlockManagement {
    pub fn new() -> Self {
        Self {
            mem_pool: Vec::new(),
            last_block: Instant::now(),
        }
    }
}

#[async_trait]
impl BlockManagement for DummyBlockManagement {
    async fn record_new_txn(
        &mut self,
        txn: Arc<Txn>,
        _ctx: Arc<TxnCtx>,
    ) -> Result<bool, CopycatError> {
        self.mem_pool.push(txn);
        Ok(true)
    }

    async fn prepare_new_block(&mut self) -> Result<bool, CopycatError> {
        Ok(true)
    }

    async fn wait_to_propose(&self) -> Result<(), CopycatError> {
        loop {
            let now = Instant::now();
            if self.mem_pool.get_size() >= 0x1000000
                || now - self.last_block >= Duration::from_secs(10)
            {
                return Ok(());
            }
        }
    }

    async fn get_new_block(&mut self) -> Result<(Arc<Block>, Arc<BlkCtx>), CopycatError> {
        let new_block = Arc::new(Block {
            header: BlockHeader::Dummy,
            txns: self.mem_pool.clone(),
        });
        self.mem_pool.clear();
        let blk_ctx = Arc::new(BlkCtx::from_blk(&new_block)?);
        Ok((new_block, blk_ctx))
    }

    async fn validate_block(
        &mut self,
        block: Arc<Block>,
        ctx: Arc<BlkCtx>,
    ) -> Result<Vec<(Arc<Block>, Arc<BlkCtx>)>, CopycatError> {
        Ok(vec![(block, ctx)])
    }

    async fn handle_pmaker_msg(&mut self, _msg: Vec<u8>) -> Result<(), CopycatError> {
        Ok(())
    }

    async fn handle_peer_blk_req(
        &mut self,
        _peer: NodeId,
        _msg: Vec<u8>,
    ) -> Result<(), CopycatError> {
        unimplemented!()
    }

    fn report(&mut self) {}

    // async fn handle_peer_blk_resp(
    //     &mut self,
    //     _peer: NodeId,
    //     _blk_id: Hash,
    //     _block: Arc<Block>,
    // ) -> Result<(), CopycatError> {
    //     unimplemented!()
    // }
}
