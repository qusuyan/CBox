use super::TxnValidation;
use crate::config::DiemBasicConfig;
use crate::context::TxnCtx;
use crate::protocol::crypto::PubKey;
use crate::transaction::{DiemAccountAddress, DiemTxn, Txn};
use crate::{CopycatError, NodeId};

use async_trait::async_trait;

use std::collections::HashMap;
use std::sync::Arc;

pub struct DiemTxnValidation {
    _id: NodeId,
    accounts: HashMap<DiemAccountAddress, PubKey>,
}

impl DiemTxnValidation {
    pub fn new(id: NodeId, _config: DiemBasicConfig) -> Self {
        Self {
            _id: id,
            accounts: HashMap::new(),
        }
    }
}

#[async_trait]
impl TxnValidation for DiemTxnValidation {
    async fn validate(
        &mut self,
        txn_batch: Vec<(NodeId, (Arc<Txn>, Arc<TxnCtx>))>,
    ) -> Result<Vec<(NodeId, (Arc<Txn>, Arc<TxnCtx>))>, CopycatError> {
        let mut correct_txns = vec![];
        for (src, (txn, ctx)) in txn_batch {
            let diem_txn = match txn.as_ref() {
                Txn::Diem { txn } => txn,
                _ => unreachable!(),
            };

            match diem_txn {
                DiemTxn::Grant {
                    receiver,
                    receiver_key,
                    ..
                } => {
                    self.accounts.insert(*receiver, receiver_key.clone());
                    correct_txns.push((src, (txn, ctx)))
                }
                DiemTxn::Txn {
                    sender, sender_key, ..
                } => {
                    // assert that the provided pubkey is the same as account's pubkey
                    if let Some(key) = self.accounts.get(sender) {
                        if sender_key == key {
                            // we have already verified signature
                            correct_txns.push((src, (txn, ctx)))
                        }
                    }
                }
            }
        }

        Ok(correct_txns)
    }
}
