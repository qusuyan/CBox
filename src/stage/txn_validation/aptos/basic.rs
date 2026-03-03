use crate::context::TxnCtx;
use crate::stage::txn_validation::TxnValidation;
use crate::transaction::{get_aptos_addr, AptosTxn, Txn};
use crate::{CopycatError, NodeId};

use async_trait::async_trait;

use std::sync::Arc;

pub struct AptosTxnValidation {
    _id: NodeId,
}

impl AptosTxnValidation {
    pub fn new(id: NodeId) -> Self {
        Self { _id: id }
    }
}

#[async_trait]
impl TxnValidation for AptosTxnValidation {
    async fn validate(
        &mut self,
        txn_batch: Vec<(NodeId, (Arc<Txn>, Arc<TxnCtx>))>,
    ) -> Result<Vec<(NodeId, (Arc<Txn>, Arc<TxnCtx>))>, CopycatError> {
        let mut correct_txns = vec![];
        for (src, (txn, txn_ctx)) in txn_batch {
            let aptos_txn = match txn.as_ref() {
                Txn::Aptos { txn } => txn,
                _ => unreachable!(),
            };

            if let AptosTxn::Txn {
                sender, sender_key, ..
            } = aptos_txn
            {
                if get_aptos_addr(&sender_key)
                    .map(|addr| addr == *sender)
                    .unwrap_or(false)
                {
                    correct_txns.push((src, (txn, txn_ctx)));
                }
            }
        }

        Ok(correct_txns)
    }
}
