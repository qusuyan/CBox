use crate::config::AvalancheVoteNoConfig;
use crate::stage::txn_validation::TxnValidation;
use crate::transaction::Txn;
use crate::{CopycatError, NodeId, TxnCtx};

use std::sync::Arc;

use async_trait::async_trait;

pub struct AvalancheVoteNoTxnValidation {
    _id: NodeId,
}

impl AvalancheVoteNoTxnValidation {
    pub fn new(id: NodeId, _config: AvalancheVoteNoConfig) -> Self {
        AvalancheVoteNoTxnValidation { _id: id }
    }
}

#[async_trait]
impl TxnValidation for AvalancheVoteNoTxnValidation {
    async fn validate(
        &mut self,
        _txn_batch: Vec<(NodeId, (Arc<Txn>, Arc<TxnCtx>))>,
    ) -> Result<Vec<(NodeId, (Arc<Txn>, Arc<TxnCtx>))>, CopycatError> {
        // this node does not propose new blocks so no need to let any txns get through
        Ok(vec![])
    }
}
