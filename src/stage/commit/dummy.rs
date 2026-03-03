use super::Commit;
use crate::context::TxnCtx;
use crate::transaction::Txn;
use crate::utils::CopycatError;

use async_trait::async_trait;

use std::sync::Arc;

pub struct DummyCommit {}

impl DummyCommit {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Commit for DummyCommit {
    async fn commit(
        &mut self,
        txn_batch: Vec<Arc<Txn>>,
        _txn_batch_ctx: Vec<Arc<TxnCtx>>,
    ) -> Result<Vec<Arc<Txn>>, CopycatError> {
        Ok(txn_batch)
    }
}
