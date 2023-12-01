use super::Decision;
use copycat_protocol::block::{Block, BlockHeader};
use copycat_protocol::crypto::{sha256, Hash};
use copycat_utils::CopycatError;
use tokio::sync::Notify;

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use async_trait::async_trait;

const COMMITTED_LENGTH: usize = 6;

pub struct BitcoinDecision {
    block_pool: HashMap<Hash, (Arc<Block>, u64)>,
    chain_tail: VecDeque<Hash>,
    notify: Notify,
}

impl BitcoinDecision {
    pub fn new() -> Self {
        Self {
            block_pool: HashMap::new(),
            chain_tail: VecDeque::new(),
            notify: Notify::new(),
        }
    }
}

#[async_trait]
impl Decision for BitcoinDecision {
    async fn new_tail(&mut self, new_tail: Vec<Arc<Block>>) -> Result<(), CopycatError> {
        // find height of first block
        let (ancester_hash, first_block_height) = match new_tail.first() {
            Some(blk) => {
                let prev_hash = match &blk.header {
                    BlockHeader::Bitcoin { prev_hash, .. } => prev_hash,
                    _ => unreachable!(),
                };
                if prev_hash.is_empty() {
                    (prev_hash, 1u64) // first block of chain
                } else {
                    match self.block_pool.get(prev_hash) {
                        Some((_, height)) => (prev_hash, height + 1),
                        None => unimplemented!(),
                    }
                }
            }
            None => return Ok(()), // no new block
        };

        loop {
            // the entire new tail has been pruned
            if self.chain_tail.is_empty() {
                break;
            }

            let old_tail = self.chain_tail.pop_back().unwrap();
            let (_, old_height) = self.block_pool.get(&old_tail).unwrap();
            if *old_height >= first_block_height {
                // old tail get overwritten
                continue;
            } else if *old_height == first_block_height - 1 {
                // found the common ancester
                if old_tail == *ancester_hash {
                    break;
                } else {
                    unreachable!("got new tail but its parent does not match existing chain");
                }
            } else {
                unreachable!("got new tail bu its parent was skipped over");
            }
        }

        for (idx, blk) in new_tail.iter().enumerate() {
            let serialized = bincode::serialize(blk)?;
            let blk_hash = sha256(&serialized)?;
            let blk_height = first_block_height + idx as u64;
            self.block_pool
                .insert(blk_hash.clone(), (blk.clone(), blk_height));
            self.chain_tail.push_back(blk_hash);
        }

        Ok(())
    }

    async fn commit_ready(&self) -> Result<(), CopycatError> {
        loop {
            if self.chain_tail.len() > COMMITTED_LENGTH {
                return Ok(());
            }
            self.notify.notified().await;
        }
    }

    async fn next_to_commit(&mut self) -> Result<Arc<Block>, CopycatError> {
        match self.chain_tail.pop_front() {
            Some(blk_hash) => {
                let (blk, _) = self.block_pool.get(&blk_hash).unwrap();
                Ok(blk.clone())
            }
            None => unreachable!(),
        }
    }
}
