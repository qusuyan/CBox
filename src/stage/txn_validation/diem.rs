use super::TxnValidation;
use crate::config::DiemConfig;
use crate::context::TxnCtx;
use crate::transaction::Txn;
use crate::{CopycatError, NodeId};

use async_trait::async_trait;

use std::sync::Arc;

pub struct DiemTxnValidation {
    _id: NodeId,
}

impl DiemTxnValidation {
    pub fn new(id: NodeId, _config: DiemConfig) -> Self {
        Self { _id: id }
    }
}

#[async_trait]
impl TxnValidation for DiemTxnValidation {
    async fn validate(
        &mut self,
        _txn_batch: Vec<(NodeId, (Arc<Txn>, Arc<TxnCtx>))>,
    ) -> Result<Vec<(NodeId, (Arc<Txn>, Arc<TxnCtx>))>, CopycatError> {
        todo!()
    }
}
