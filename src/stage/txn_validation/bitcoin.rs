use super::TxnValidation;
use crate::config::BitcoinConfig;
use crate::protocol::crypto::Hash;
use crate::transaction::{BitcoinTxn, Txn};
use crate::{CopycatError, NodeId, TxnCtx};

use async_trait::async_trait;

use std::collections::HashMap;
use std::sync::Arc;

pub struct BitcoinTxnValidation {
    _id: NodeId,
    txn_pool: HashMap<Hash, (Arc<Txn>, Arc<TxnCtx>)>,
    pending_txns: HashMap<Hash, HashMap<Hash, ((Arc<Txn>, Arc<TxnCtx>), NodeId)>>,
}

impl BitcoinTxnValidation {
    pub fn new(id: NodeId, _config: BitcoinConfig) -> Self {
        BitcoinTxnValidation {
            _id: id,
            txn_pool: HashMap::new(),
            pending_txns: HashMap::new(),
        }
    }

    fn validate_inner(
        &mut self,
        src: NodeId,
        txn: Arc<Txn>,
        ctx: Arc<TxnCtx>,
    ) -> Vec<(NodeId, (Arc<Txn>, Arc<TxnCtx>))> {
        let txn_hash = ctx.id;

        // duplicate - already checked
        if self.txn_pool.contains_key(&txn_hash) {
            return vec![];
        }

        let btc_txn = match txn.as_ref() {
            Txn::Bitcoin { txn } => txn,
            _ => unreachable!(),
        };

        match self.validate_txn(btc_txn) {
            Ok(valid) => {
                if !valid {
                    return vec![];
                }
            }
            Err(missing_deps) => {
                for missing_dep in missing_deps {
                    let siblings = self
                        .pending_txns
                        .entry(missing_dep)
                        .or_insert(HashMap::new());
                    siblings
                        .entry(txn_hash)
                        .or_insert(((txn.clone(), ctx.clone()), src));
                }

                // Do not enqueue the txn yet since it might be invalid
                return vec![];
            }
        };

        // got a new valid txn
        self.txn_pool.insert(txn_hash, (txn.clone(), ctx.clone()));

        // add txn to returned vector
        let mut valid_offsprings = vec![];
        valid_offsprings.push((src, (txn.clone(), ctx.clone())));

        // add any pending childrens
        if let Some(children) = self.pending_txns.remove(&txn_hash) {
            for (_, ((child_txn, child_ctx), child_srcs)) in children {
                valid_offsprings.extend(self.validate_inner(child_srcs, child_txn, child_ctx));
            }
        }

        valid_offsprings
    }

    fn validate_txn(&self, btc_txn: &BitcoinTxn) -> Result<bool, Vec<Hash>> {
        match btc_txn {
            BitcoinTxn::Send {
                sender,
                in_utxo,
                out_utxo,
                remainder,
                ..
            } => {
                let mut in_utxo_txns = vec![];
                let mut missing_deps = vec![];
                for in_utxo_hash in in_utxo.iter() {
                    match self.txn_pool.get(in_utxo_hash) {
                        Some((txn, _)) => match txn.as_ref() {
                            Txn::Bitcoin { txn } => in_utxo_txns.push(txn),
                            _ => unreachable!(),
                        },
                        None => missing_deps.push(in_utxo_hash.clone()),
                    };
                }

                // if missing deps, put into pending_txns
                if missing_deps.len() > 0 {
                    return Err(missing_deps);
                }

                // compute total input values
                let mut input_value = 0;
                for utxo in in_utxo_txns.into_iter() {
                    // add values together to find total input value
                    let value = match utxo {
                        BitcoinTxn::Grant { out_utxo, receiver } => {
                            if receiver == sender {
                                out_utxo
                            } else {
                                return Ok(false); // utxo does not belong to sender
                            }
                        }
                        BitcoinTxn::Incentive { out_utxo, receiver } => {
                            if receiver == sender {
                                out_utxo
                            } else {
                                return Ok(false); // utxo does not belong to sender
                            }
                        }
                        BitcoinTxn::Send {
                            sender: parent_txn_sender,
                            receiver: parent_txn_recver,
                            out_utxo: parent_txn_out_utxo,
                            remainder: parent_txn_remainder,
                            ..
                        } => {
                            if parent_txn_recver == sender {
                                parent_txn_out_utxo
                            } else if parent_txn_sender == sender {
                                parent_txn_remainder
                            } else {
                                return Ok(false); // utxo does not belong to sender
                            }
                        }
                    };
                    input_value += value;
                }

                // check if the input values match output values
                if input_value != out_utxo + remainder {
                    return Ok(false); // input and output utxo values do not match
                }

                Ok(true)
            }
            BitcoinTxn::Grant { .. } => {
                // grant txns are always correct
                Ok(true)
            }
            BitcoinTxn::Incentive { .. } => {
                // this are generated for protocol and hence will not come from clients
                unreachable!();
            }
        }
    }
}

#[async_trait]
impl TxnValidation for BitcoinTxnValidation {
    async fn validate(
        &mut self,
        txn_batch: Vec<(NodeId, (Arc<Txn>, Arc<TxnCtx>))>,
    ) -> Result<Vec<(NodeId, (Arc<Txn>, Arc<TxnCtx>))>, CopycatError> {
        let mut correct_txns = vec![];

        for (src, (txn, ctx)) in txn_batch {
            correct_txns.extend(self.validate_inner(src, txn, ctx));
        }

        Ok(correct_txns)
    }
}
