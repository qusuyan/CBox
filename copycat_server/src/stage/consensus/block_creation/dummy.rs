use async_trait::async_trait;

use super::BlockCreation;
use crate::chain::DummyBlock;
use copycat_utils::CopycatError;

use std::sync::Arc;

pub struct DummyBlockCreation<TxnType> {
    mem_pool: Vec<Arc<TxnType>>,
}

impl<TxnType> DummyBlockCreation<TxnType> {
    pub fn new() -> Self {
        Self {
            mem_pool: Vec::new(),
        }
    }
}

#[async_trait]
impl<TxnType> BlockCreation<TxnType, DummyBlock<TxnType>> for DummyBlockCreation<TxnType>
where
    TxnType: Sync + Send,
{
    async fn new_txn(&mut self, txn: Arc<TxnType>) -> Result<(), CopycatError> {
        self.mem_pool.push(txn);
        Ok(())
    }

    async fn new_block(
        &mut self,
        _pmaker_msg: Arc<Vec<u8>>,
    ) -> Result<Arc<DummyBlock<TxnType>>, CopycatError> {
        let txns = self.mem_pool.drain(0..self.mem_pool.len());
        Ok(Arc::new(DummyBlock::new(txns.collect())))
    }
}
