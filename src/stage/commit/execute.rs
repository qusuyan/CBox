use super::Commit;
use crate::protocol::crypto::vector_snark::DummyMerkleTree;
use crate::stage::DelayPool;
use crate::transaction::{get_aptos_addr, AptosAccountAddress, AptosTxn, Txn};
use crate::{CopycatError, NodeId};

use async_trait::async_trait;

use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::Arc;

pub struct ExecuteCommit {
    _id: NodeId,
    balance: HashMap<AptosAccountAddress, u64>,
    state_merkle: DummyMerkleTree,
    delay: Arc<DelayPool>,
}

impl ExecuteCommit {
    pub fn new(id: NodeId, delay: Arc<DelayPool>) -> Self {
        Self {
            _id: id,
            balance: HashMap::new(),
            state_merkle: DummyMerkleTree::new(),
            delay,
        }
    }
}

#[async_trait]
impl Commit for ExecuteCommit {
    async fn commit(&mut self, block: &Vec<Arc<Txn>>) -> Result<(), CopycatError> {
        let mut inserts = 0usize;
        let mut updates = 0usize;
        let mut exec_time = 0f64;
        for txn in block {
            let aptos_txn = match txn.as_ref() {
                Txn::Aptos { txn } => txn,
                _ => unreachable!(),
            };

            match aptos_txn {
                AptosTxn::Grant {
                    receiver_key,
                    amount,
                } => {
                    let addr = get_aptos_addr(receiver_key)?;
                    match self.balance.entry(addr) {
                        Entry::Occupied(mut e) => {
                            *e.get_mut() += amount;
                            updates += 1;
                        }
                        Entry::Vacant(e) => {
                            e.insert(*amount);
                            inserts += 1;
                        }
                    }
                }

                AptosTxn::Txn {
                    sender,
                    seqno: _seqno, // TODO
                    payload,
                    max_gas_amount,
                    ..
                } => {
                    let sender_balance = match self.balance.get(sender) {
                        Some(account) => account,
                        None => continue, // invalid txn - unknown sender
                    };

                    if sender_balance < max_gas_amount {
                        continue; // invalid txn - not enough balance
                    }

                    exec_time += payload.script_runtime_sec;
                    if !payload.script_succeed {
                        continue; // invalid txn
                    }
                    // +1 for sender paying gas from balance
                    updates += payload.distinct_writes + 1;
                }
            }
        }

        let insert_dur = self.state_merkle.append(inserts)?;
        let update_dur = self.state_merkle.append(updates)?;
        self.delay
            .process_illusion(insert_dur + update_dur + exec_time)
            .await;

        Ok(())
    }
}
