use crate::protocol::crypto::Hash;

use std::sync::Arc;

#[derive(Debug)]
pub struct TxnCtx {
    pub id: Hash,
}

#[derive(Debug)]
pub struct BlkCtx {
    pub id: Hash,
    pub txn_ctx: Vec<Arc<TxnCtx>>,
}
