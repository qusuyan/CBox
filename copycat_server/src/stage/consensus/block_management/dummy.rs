use super::BlockManagement;
use copycat_protocol::block::{Block, DummyBlock};
use copycat_protocol::transaction::Txn;
use copycat_utils::CopycatError;

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
    async fn record_new_txn(&mut self, txn: Arc<Txn>) -> Result<bool, CopycatError> {
        self.mem_pool.push(txn);
        Ok(true)
    }

    async fn prepare_new_block(&mut self) -> Result<(), CopycatError> {
        Ok(())
    }

    async fn wait_to_propose(&mut self) -> Result<(), CopycatError> {
        loop {
            let now = Instant::now();
            if self.mem_pool.get_size() >= 0x1000000
                || now - self.last_block >= Duration::from_secs(10)
            {
                return Ok(());
            }
        }
    }

    async fn get_new_block(&mut self) -> Result<Arc<Block>, CopycatError> {
        let new_block = Arc::new(Block::Dummy {
            blk: DummyBlock {
                txns: self.mem_pool.clone(),
            },
        });
        self.mem_pool.clear();
        Ok(new_block)
    }

    async fn validate_block(&mut self, block: Arc<Block>) -> Result<Vec<Arc<Block>>, CopycatError> {
        Ok(vec![block])
    }

    async fn handle_pmaker_msg(&mut self, _msg: Arc<Vec<u8>>) -> Result<(), CopycatError> {
        Ok(())
    }
}
