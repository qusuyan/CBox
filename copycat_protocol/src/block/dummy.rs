use crate::transaction::Txn;

use super::BlockTrait;

use get_size::GetSize;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, GetSize)]
pub struct DummyBlock {
    txns: Vec<Arc<Txn>>,
}

impl DummyBlock {
    pub fn new(txns: Vec<Arc<Txn>>) -> Self {
        Self { txns }
    }
}

impl BlockTrait for DummyBlock {
    fn fetch_txns(&self) -> Vec<Arc<Txn>> {
        self.txns.clone()
    }
}
