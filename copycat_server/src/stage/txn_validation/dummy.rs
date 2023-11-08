use copycat_utils::CopycatError;

use super::TxnValidation;

use async_trait::async_trait;

pub struct DummyTxnValidation {}

impl DummyTxnValidation {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl<TxnType> TxnValidation<TxnType> for DummyTxnValidation {
    async fn validate(&self, _txn: &TxnType) -> Result<bool, CopycatError> {
        Ok(true)
    }
}
