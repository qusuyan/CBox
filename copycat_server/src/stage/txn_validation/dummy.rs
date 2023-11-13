use super::TxnValidation;
use copycat_protocol::transaction::Txn;
use copycat_utils::CopycatError;

use async_trait::async_trait;

pub struct DummyTxnValidation {}

impl DummyTxnValidation {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl TxnValidation for DummyTxnValidation {
    async fn validate(&self, _txn: &Txn) -> Result<bool, CopycatError> {
        Ok(true)
    }
}
