use copycat_utils::{CopycatError, NodeId};

use super::TxnValidation;

use async_trait::async_trait;

pub struct DummyTxnValidation {
    _id: NodeId,
}

impl DummyTxnValidation {
    pub fn new(id: NodeId) -> Self {
        Self { _id: id }
    }
}

#[async_trait]
impl<TxnType> TxnValidation<TxnType> for DummyTxnValidation {
    async fn validate(&self, _txn: &TxnType) -> Result<bool, CopycatError> {
        Ok(true)
    }
}
