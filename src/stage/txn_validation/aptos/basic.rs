use crate::context::TxnCtx;
use crate::stage::txn_validation::TxnValidation;
use crate::transaction::Txn;
use crate::{CopycatError, NodeId};

use async_trait::async_trait;

use std::sync::Arc;

pub struct AptosTxnValidation {
    id: NodeId,
}

impl AptosTxnValidation {
    pub fn new(id: NodeId) -> Self {
        Self { id }
    }
}

#[async_trait]
impl TxnValidation for AptosTxnValidation {
    async fn validate(
        &mut self,
        txn_batch: Vec<(NodeId, (Arc<Txn>, Arc<TxnCtx>))>,
    ) -> Result<Vec<(NodeId, (Arc<Txn>, Arc<TxnCtx>))>, CopycatError> {
        todo!()
    }
}
