use super::BlockTrait;

use get_size::GetSize;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, GetSize)]
pub struct DummyBlock<TxnType> {
    txns: Vec<Arc<TxnType>>,
}

impl<TxnType> DummyBlock<TxnType> {
    pub fn new(txns: Vec<Arc<TxnType>>) -> Self {
        Self { txns }
    }
}

impl<TxnType> BlockTrait<TxnType> for DummyBlock<TxnType>
where
    TxnType: Sync + Send,
{
    fn fetch_txns(&self) -> Vec<Arc<TxnType>> {
        self.txns.clone()
    }
}
