use super::TxnValidation;
use crate::protocol::transaction::Txn;
use crate::{CopycatError, NodeId, TxnCtx};

use async_trait::async_trait;

use std::sync::Arc;

pub struct DummyTxnValidation {
    _id: NodeId,
}

impl DummyTxnValidation {
    pub fn new(id: NodeId) -> Self {
        DummyTxnValidation { _id: id }
    }
}

#[async_trait]
impl TxnValidation for DummyTxnValidation {
    async fn validate(
        &mut self,
        txn_batch: Vec<(NodeId, (Arc<Txn>, Arc<TxnCtx>))>,
    ) -> Result<Vec<(NodeId, (Arc<Txn>, Arc<TxnCtx>))>, CopycatError> {
        Ok(txn_batch)
    }
}
