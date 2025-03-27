use super::{BlockManagement, CurBlockState};
use crate::config::ChainReplicationConfig;
use crate::context::{BlkCtx, TxnCtx};
use crate::protocol::block::{Block, BlockHeader};
use crate::protocol::crypto::Hash;
use crate::protocol::transaction::Txn;
use crate::utils::{CopycatError, NodeId};

use async_trait::async_trait;
use get_size::GetSize;
use primitive_types::U256;

use std::sync::Arc;
use tokio::sync::Notify;

pub struct ChainReplicationBlockManagement {
    id: NodeId,
    mem_pool: Vec<(Arc<Txn>, Arc<TxnCtx>)>,
    is_head: bool,
    block_size: usize,
    // next block info
    next_blk_id: U256,
    curr_batch_len: usize,
    curr_batch_size: usize,
    _notify: Notify,
}

impl ChainReplicationBlockManagement {
    pub fn new(id: NodeId, config: ChainReplicationConfig) -> Self {
        let is_head = config.order[0] == id;
        Self {
            id,
            mem_pool: vec![],
            is_head,
            block_size: config.blk_size,
            next_blk_id: U256::one(),
            curr_batch_len: 0,
            curr_batch_size: 0,
            _notify: Notify::new(),
        }
    }
}

#[async_trait]
impl BlockManagement for ChainReplicationBlockManagement {
    async fn record_new_txn(
        &mut self,
        txn: Arc<Txn>,
        ctx: Arc<TxnCtx>,
    ) -> Result<bool, CopycatError> {
        if !self.is_head {
            pf_trace!(self.id; "Non-head ignoring txn {:?}", txn);
            return Ok(true);
        }

        self.mem_pool.push((txn, ctx));
        Ok(true)
    }

    async fn prepare_new_block(&mut self) -> Result<CurBlockState, CopycatError> {
        if !self.is_head {
            return Ok(CurBlockState::Full);
        }

        let blk_full = loop {
            if self.curr_batch_size >= self.block_size {
                break CurBlockState::Full;
            }

            if self.curr_batch_len >= self.mem_pool.len() {
                break CurBlockState::EmptyMempool;
            }

            let (next_txn, _) = &self.mem_pool[self.curr_batch_len];
            self.curr_batch_len += 1;
            self.curr_batch_size += next_txn.get_size();
        };

        // pf_debug!(self.id; "blk_full: {}, curr_batch_size: {}, curr_batch_len: {}, mem_pool size: {}", blk_full, self.curr_batch_size, self.curr_batch_len, self.mem_pool.len());
        Ok(blk_full)
    }

    async fn wait_to_propose(&self) -> Result<(), CopycatError> {
        loop {
            if self.is_head && self.curr_batch_size >= self.block_size {
                return Ok(());
            }
            self._notify.notified().await;
        }
    }

    async fn get_new_block(&mut self) -> Result<(Arc<Block>, Arc<BlkCtx>), CopycatError> {
        assert!(self.is_head);
        let txns_with_ctx = self.mem_pool.drain(0..self.curr_batch_len);
        let (txns, txn_ctx) = txns_with_ctx.unzip();

        let header = BlockHeader::ChainReplication {
            blk_id: Hash(self.next_blk_id),
        };
        let blk_ctx: Arc<BlkCtx> = Arc::new(BlkCtx::from_header_and_txns(&header, txn_ctx)?);
        let blk = Arc::new(Block { header, txns });

        self.curr_batch_len = 0;
        self.curr_batch_size = 0;
        self.next_blk_id += U256::one();

        Ok((blk, blk_ctx))
    }

    async fn validate_block(
        &mut self,
        _src: NodeId,
        block: Arc<Block>,
        ctx: Arc<BlkCtx>,
    ) -> Result<Vec<(Arc<Block>, Arc<BlkCtx>)>, CopycatError> {
        assert!(!self.is_head);
        assert!(matches!(block.header, BlockHeader::ChainReplication { .. }));
        return Ok(vec![(block, ctx)]);
    }

    async fn handle_pmaker_msg(&mut self, _msg: Vec<u8>) -> Result<(), CopycatError> {
        unreachable!()
    }

    async fn handle_peer_blk_req(
        &mut self,
        _peer: NodeId,
        _msg: Vec<u8>,
    ) -> Result<(), CopycatError> {
        unreachable!()
    }

    fn report(&mut self) {}
}
