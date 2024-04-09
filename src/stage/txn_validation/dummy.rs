use super::TxnValidation;
use crate::context::TxnCtx;
use crate::protocol::transaction::Txn;
use crate::utils::CopycatError;

use async_trait::async_trait;
use primitive_types::U256;

use std::sync::Arc;

pub struct DummyTxnValidation {}

impl DummyTxnValidation {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl TxnValidation for DummyTxnValidation {
    async fn validate(&mut self, _txn: Arc<Txn>) -> Result<Option<Arc<TxnCtx>>, CopycatError> {
        Ok(Some(Arc::new(TxnCtx { id: U256::zero() })))
    }
}
