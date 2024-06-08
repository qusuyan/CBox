use super::BlockManagement;
use crate::config::ChainReplicationConfig;
use crate::context::{BlkCtx, TxnCtx};
use crate::protocol::block::{Block, BlockHeader};
use crate::protocol::crypto::Hash;
use crate::protocol::transaction::Txn;
use crate::utils::{CopycatError, NodeId};

use async_trait::async_trait;
use get_size::GetSize;

use std::sync::Arc;
use tokio::sync::Notify;

pub struct ChainReplicationBlockManagement {
    mem_pool: Vec<(Arc<Txn>, Arc<TxnCtx>)>,
    is_head: bool,
    block_size: usize,
    // next block info
    next_blk_id: Hash,
    curr_batch_len: usize,
    curr_batch_size: usize,
    _notify: Notify,
}

impl ChainReplicationBlockManagement {
    pub fn new(me: NodeId, config: ChainReplicationConfig) -> Self {
        let is_head = config.order[0] == me;
        Self {
            mem_pool: vec![],
            is_head,
            block_size: config.blk_size,
            next_blk_id: Hash::zero(),
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
        self.mem_pool.push((txn, ctx));
        Ok(true)
    }

    async fn prepare_new_block(&mut self) -> Result<bool, CopycatError> {
        let blk_full = loop {
            if self.curr_batch_size >= self.block_size {
                break true;
            }

            if self.curr_batch_len > self.mem_pool.len() {
                break false;
            }

            let (next_txn, _) = &self.mem_pool[self.curr_batch_len];
            self.curr_batch_len += 1;
            self.curr_batch_size += next_txn.get_size();
        };

        Ok(blk_full)
    }

    async fn wait_to_propose(&self) -> Result<(), CopycatError> {
        loop {
            if self.curr_batch_size >= self.block_size {
                return Ok(());
            }
            self._notify.notified().await;
        }
    }

    async fn get_new_block(&mut self) -> Result<(Arc<Block>, Arc<BlkCtx>), CopycatError> {
        let txns_with_ctx = self.mem_pool.drain(0..self.curr_batch_len);
        let (txns, txn_ctx) = txns_with_ctx.unzip();

        let blk = Arc::new(Block {
            header: BlockHeader::ChainReplication {
                blk_id: self.next_blk_id,
            },
            txns,
        });
        let blk_ctx = Arc::new(BlkCtx::from_header_and_txns(&BlockHeader::Dummy, txn_ctx)?);

        self.next_blk_id += Hash::one();

        Ok((blk, blk_ctx))
    }

    async fn validate_block(
        &mut self,
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
