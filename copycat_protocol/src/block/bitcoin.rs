use crate::crypto::Hash;
use crate::transaction::Txn;

use std::sync::Arc;

use get_size::GetSize;
use serde::{Deserialize, Serialize};

use super::BlockTrait;

#[derive(Clone, Debug, Serialize, Deserialize, GetSize)]
pub struct BitcoinBlock {
    pub prev_hash: Hash, // vec![] if first block of chain
    pub nonce: u32,      // https://en.bitcoin.it/wiki/Nonce
    pub txns: Vec<Arc<Txn>>,
}

impl BitcoinBlock {
    pub fn new(prev_hash: Hash) -> Self {
        Self {
            prev_hash,
            nonce: 0,
            txns: vec![],
        }
    }
}

impl BlockTrait for BitcoinBlock {
    fn fetch_txns(&self) -> Vec<Arc<Txn>> {
        return self.txns.clone();
    }
}
