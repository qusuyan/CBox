use super::TxnValidation;
use copycat_protocol::transaction::Txn;
use copycat_utils::CopycatError;

use async_trait::async_trait;

use std::sync::Arc;

pub struct DummyTxnValidation {}

impl DummyTxnValidation {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl TxnValidation for DummyTxnValidation {
    async fn validate(&mut self, _txn: Arc<Txn>) -> Result<bool, CopycatError> {
        Ok(true)
    }
}
