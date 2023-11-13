use super::BlockCreation;
use copycat_protocol::block::{Block, DummyBlock};
use copycat_protocol::transaction::Txn;
use copycat_utils::CopycatError;

use async_trait::async_trait;

use std::collections::HashSet;
use std::sync::Arc;
use tokio::time::{Duration, Instant};

pub struct DummyBlockCreation {
    mem_pool: HashSet<Arc<Txn>>,
    last_block: Instant,
}

impl DummyBlockCreation {
    pub fn new() -> Self {
        Self {
            mem_pool: HashSet::new(),
            last_block: Instant::now(),
        }
    }
}

#[async_trait]
impl BlockCreation for DummyBlockCreation {
    async fn new_txn(&mut self, txn: Arc<Txn>) -> Result<(), CopycatError> {
        self.mem_pool.insert(txn);
        Ok(())
    }

    async fn new_block(&mut self) -> Result<Arc<Block>, CopycatError> {
        loop {
            let now = Instant::now();
            if self.mem_pool.len() >= 1000 || now - self.last_block >= Duration::from_secs(10) {
                self.last_block = now;
                let txns = self.mem_pool.drain();
                return Ok(Arc::new(Block::Dummy {
                    blk: DummyBlock::new(txns.collect()),
                }));
            }
            tokio::task::yield_now().await;
        }
    }
}
