use async_trait::async_trait;
use tokio::time::{Duration, Instant};

use super::BlockCreation;
use copycat_protocol::block::{Block, DummyBlock};
use copycat_utils::CopycatError;

use std::collections::HashSet;
use std::hash::Hash;
use std::sync::Arc;

pub struct DummyBlockCreation<TxnType> {
    mem_pool: HashSet<Arc<TxnType>>,
    last_block: Instant,
}

impl<TxnType> DummyBlockCreation<TxnType> {
    pub fn new() -> Self {
        Self {
            mem_pool: HashSet::new(),
            last_block: Instant::now(),
        }
    }
}

#[async_trait]
impl<TxnType> BlockCreation<TxnType> for DummyBlockCreation<TxnType>
where
    TxnType: Sync + Send + Eq + Hash,
{
    async fn new_txn(&mut self, txn: Arc<TxnType>) -> Result<(), CopycatError> {
        self.mem_pool.insert(txn);
        Ok(())
    }

    async fn new_block(&mut self) -> Result<Arc<Block<TxnType>>, CopycatError> {
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
