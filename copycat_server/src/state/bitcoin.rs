use copycat_protocol::crypto::Hash;
use copycat_protocol::transaction::Txn;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct BitcoinState {
    pub txn_pool: RwLock<HashMap<Hash, Arc<Txn>>>,
    pub pending_txn: RwLock<Vec<Hash>>,
    pub committed_txns: RwLock<Vec<Hash>>,
}

impl BitcoinState {
    pub fn new() -> Self {
        Self {
            txn_pool: RwLock::new(HashMap::new()),
            pending_txn: RwLock::new(vec![]),
            committed_txns: RwLock::new(vec![]),
        }
    }
}
