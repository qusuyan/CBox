use super::Decision;
use crate::config::BitcoinBasicConfig;
use crate::context::{BlkCtx, TxnCtx};
use crate::protocol::block::{Block, BlockHeader};
use crate::protocol::crypto::Hash;
use crate::transaction::Txn;
use crate::utils::CopycatError;
use crate::NodeId;
use tokio::sync::Notify;

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use async_trait::async_trait;

pub struct BitcoinDecision {
    id: NodeId,
    commit_len: u8,
    block_pool: HashMap<Hash, (Arc<Block>, Arc<BlkCtx>, u64)>,
    chain_tail: VecDeque<Hash>,
    _notify: Notify,
    first_block_seen: bool,
    miners_blk_cnt: HashMap<NodeId, usize>,
}

impl BitcoinDecision {
    pub fn new(id: NodeId, config: BitcoinBasicConfig) -> Self {
        Self {
            id,
            commit_len: config.commit_depth,
            block_pool: HashMap::new(),
            chain_tail: VecDeque::new(),
            _notify: Notify::new(),
            first_block_seen: false,
            miners_blk_cnt: HashMap::new(),
        }
    }
}

#[async_trait]
impl Decision for BitcoinDecision {
    async fn new_tail(
        &mut self,
        _src: NodeId,
        new_tail: Vec<(Arc<Block>, Arc<BlkCtx>)>,
    ) -> Result<(), CopycatError> {
        // find height of first block
        let (ancester_hash, first_block_height) = match new_tail.first() {
            Some((blk, _)) => {
                let parent_id = match &blk.header {
                    BlockHeader::Bitcoin { parent_id, .. } => parent_id,
                    _ => unreachable!(),
                };
                if parent_id.0.is_zero() {
                    (parent_id, 1u64) // first block of chain
                } else {
                    match self.block_pool.get(parent_id) {
                        Some((_, _, height)) => (parent_id, height + 1),
                        None => unimplemented!(),
                    }
                }
            }
            None => return Ok(()), // no new block
        };

        loop {
            // the entire new tail has been pruned
            if self.chain_tail.is_empty() {
                if self.first_block_seen {
                    pf_warn!(
                        self.id; "entire undecided chain pruned, a committed block might need to be undone"
                    );
                } else {
                    self.first_block_seen = true;
                }
                break;
            }

            let old_tail = self.chain_tail.back().unwrap();
            let (_, _, old_height) = self.block_pool.get(&old_tail).unwrap();
            if *old_height >= first_block_height {
                // old tail get overwritten
                self.chain_tail.pop_back();
            } else if *old_height == first_block_height - 1 {
                // found the common ancester
                if old_tail == ancester_hash {
                    break;
                } else {
                    unreachable!("got new tail but its parent does not match existing chain");
                }
            } else {
                unreachable!("got new tail but its parent was skipped over");
            }
        }

        for (idx, (blk, blk_ctx)) in new_tail.iter().enumerate() {
            let blk_hash = blk_ctx.id;
            let blk_height = first_block_height + idx as u64;
            self.block_pool
                .insert(blk_hash.clone(), (blk.clone(), blk_ctx.clone(), blk_height));
            self.chain_tail.push_back(blk_hash);
        }

        pf_debug!(
            self.id;
            "Current chain length: {}, # blocks waiting to be committed: {}",
            first_block_height + new_tail.len() as u64,
            self.chain_tail.len()
        );

        Ok(())
    }

    async fn commit_ready(&self) -> Result<(), CopycatError> {
        loop {
            if self.chain_tail.len() > self.commit_len as usize {
                return Ok(());
            }
            self._notify.notified().await;
        }
    }

    async fn next_to_commit(
        &mut self,
    ) -> Result<(u64, (Vec<Arc<Txn>>, Vec<Arc<TxnCtx>>)), CopycatError> {
        match self.chain_tail.pop_front() {
            Some(blk_hash) => {
                let (blk, blk_ctx, height) = self.block_pool.get(&blk_hash).unwrap();
                let miner = match &blk.header {
                    BlockHeader::Bitcoin { proposer, .. } => proposer,
                    _ => unreachable!(),
                };
                let blk_cnt = self.miners_blk_cnt.entry(*miner).or_insert(0);
                *blk_cnt += 1;
                Ok((*height, (blk.txns.clone(), blk_ctx.txn_ctx.clone())))
            }
            None => unreachable!(),
        }
    }

    async fn timeout(&self) -> Result<(), CopycatError> {
        self._notify.notified().await;
        unreachable!();
    }

    async fn handle_timeout(&mut self) -> Result<(), CopycatError> {
        unreachable!();
    }

    async fn handle_peer_msg(
        &mut self,
        _src: NodeId,
        _content: Vec<u8>,
    ) -> Result<(), CopycatError> {
        unreachable!("Bitcoin consensus can be done locally")
    }

    fn report(&mut self) {
        pf_info!(self.id; "miner blk count: {:?}", self.miners_blk_cnt);
    }
}
