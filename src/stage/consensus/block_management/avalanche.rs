use super::BlockManagement;
use crate::config::AvalancheConfig;
use crate::peers::PeerMessenger;

use crate::protocol::block::{Block, BlockHeader};
use crate::protocol::crypto::{sha256, DummyMerkleTree, Hash, PubKey};
use crate::protocol::transaction::{AvalancheTxn, Txn};
use crate::protocol::{CryptoScheme, MsgType};
use crate::utils::{CopycatError, NodeId};

use async_trait::async_trait;
use get_size::GetSize;
use primitive_types::U256;
use rand::Rng;

use serde::Serialize;
use tokio::time::{Duration, Instant};

use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

pub struct AvalancheBlockManagement {
    id: NodeId,
    crypto_scheme: CryptoScheme,
    blk_len: usize,
    txn_pool: HashMap<Hash, Arc<Txn>>,
    pending_txns: VecDeque<Hash>,
    block_pool: HashMap<Hash, Arc<Block>>,
    utxo_blk_map: HashMap<Hash, Hash>,
    // fields for constructing new block
    curr_batch: Vec<Hash>,
    utxo_spent: HashSet<(Hash, PubKey)>,
    new_utxo: HashSet<(Hash, PubKey)>,
    batch_parents: HashSet<Hash>,
    // for requesting missing blocks
    peer_messenger: Arc<PeerMessenger>,
}

impl AvalancheBlockManagement {
    pub fn new(
        id: NodeId,
        crypto_scheme: CryptoScheme,
        config: AvalancheConfig,
        peer_messenger: Arc<PeerMessenger>,
    ) -> Self {
        Self {
            id,
            crypto_scheme,
            blk_len: config.blk_len,
            txn_pool: HashMap::new(),
            block_pool: HashMap::new(),
            pending_txns: VecDeque::new(),
            utxo_blk_map: HashMap::new(),
            curr_batch: vec![],
            utxo_spent: HashSet::new(),
            new_utxo: HashSet::new(),
            batch_parents: HashSet::new(),
            peer_messenger,
        }
    }
}

impl AvalancheBlockManagement {
    fn validate_txn(&self, txn: &AvalancheTxn) -> Result<bool, CopycatError> {
        match txn {
            AvalancheTxn::Send {
                sender: txn_sender,
                in_utxo: txn_in_utxo,
                out_utxo: txn_out_utxo,
                remainder: txn_remainder,
                ..
            } => {
                let mut input_value = 0;
                for in_utxo_hash in txn_in_utxo.iter() {
                    // first check that input transactions exists, we can check for double spend later as a block
                    // add values together to find total input value
                    let utxo = match self.txn_pool.get(in_utxo_hash) {
                        Some(txn) => match txn.as_ref() {
                            Txn::Avalanche { txn } => txn,
                            _ => unreachable!(),
                        },
                        None => return Ok(false), // invalid utxo
                    };

                    let value = match utxo {
                        AvalancheTxn::Grant { out_utxo, receiver } => {
                            if receiver == txn_sender {
                                out_utxo
                            } else {
                                return Ok(false); // utxo does not belong to sender
                            }
                        }
                        AvalancheTxn::Send {
                            sender,
                            receiver,
                            out_utxo,
                            remainder,
                            ..
                        } => {
                            if receiver == txn_sender {
                                out_utxo
                            } else if sender == txn_sender {
                                remainder
                            } else {
                                return Ok(false); // utxo does not belong to sender
                            }
                        }
                    };
                    input_value += value;
                }

                // check if the input values match output values
                if input_value != txn_out_utxo + txn_remainder {
                    return Ok(false); // input and output utxo values do not match
                }
            }
            &AvalancheTxn::Grant { .. } => {
                // bypass txn validity checks
            }
        }

        Ok(true)
    }
}

#[async_trait]
impl BlockManagement for AvalancheBlockManagement {
    async fn record_new_txn(&mut self, txn: Arc<Txn>) -> Result<bool, CopycatError> {
        let txn_hash = sha256(&bincode::serialize(txn.as_ref())?)?;
        // ignore duplicates
        if self.txn_pool.contains_key(&txn_hash) {
            return Ok(false);
        }

        let avax_txn = match txn.as_ref() {
            Txn::Avalanche { txn } => txn,
            _ => unreachable!(),
        };

        if !self.validate_txn(avax_txn)? {
            return Ok(false);
        }

        self.txn_pool.insert(txn_hash.clone(), txn.clone());
        self.pending_txns.push_back(txn_hash);

        Ok(true)
    }

    async fn prepare_new_block(&mut self) -> Result<(), CopycatError> {
        loop {
            if self.curr_batch.len() >= self.blk_len {
                break;
            }

            let next_txn_hash = match self.pending_txns.pop_front() {
                Some(txn) => txn,
                None => break,
            };
            let next_txn = match self.txn_pool.get(&next_txn_hash).unwrap().as_ref() {
                Txn::Avalanche { txn } => txn,
                _ => unreachable!(),
            };

            let valid = match next_txn {
                AvalancheTxn::Send {
                    in_utxo, sender, ..
                } => {
                    let mut valid = true;
                    for utxo in in_utxo {
                        let utxo_key = (utxo.clone(), sender.clone());
                        // another txn in conflict set in the same batch
                        if self.utxo_spent.contains(&utxo_key) {
                            valid = false;
                            break;
                        }
                        if let Some(blk_hash) = self.utxo_blk_map.get(utxo) {
                            // good case, this txn depends on a parent blk
                            self.batch_parents.insert(blk_hash.clone());
                        } else if self.new_utxo.contains(&utxo_key) {
                            // good case, this txn depends on another txn in this blk
                        } else {
                            // bad case, the previous txn not in a blk yet
                            valid = false;
                            break;
                        }
                    }
                    valid
                }
                AvalancheTxn::Grant { .. } => true,
            };

            if !valid {
                // look for the next valid txn
                self.pending_txns.push_back(next_txn_hash);
                continue;
            }

            // add txn to batch
            match next_txn {
                AvalancheTxn::Send {
                    sender,
                    in_utxo,
                    receiver,
                    ..
                } => {
                    self.new_utxo
                        .insert((next_txn_hash.clone(), receiver.clone()));
                    self.new_utxo
                        .insert((next_txn_hash.clone(), sender.clone()));
                    for utxo in in_utxo {
                        self.utxo_spent.insert((utxo.clone(), sender.clone()));
                    }
                }
                AvalancheTxn::Grant { receiver, .. } => {
                    self.new_utxo
                        .insert((next_txn_hash.clone(), receiver.clone()));
                }
            }
            self.curr_batch.push(next_txn_hash);
        }

        Ok(())
    }

    async fn wait_to_propose(&self) -> Result<(), CopycatError> {
        todo!();
    }

    async fn get_new_block(&mut self) -> Result<Arc<Block>, CopycatError> {
        let txn_hashs: Vec<Hash> = self.curr_batch.drain(0..).collect();
        let txns = txn_hashs
            .iter()
            .map(|txn_hash| self.txn_pool.get(&txn_hash).unwrap().clone())
            .collect();

        let parent_blks = self.batch_parents.drain().collect();
        let header = BlockHeader::Avalanche {
            proposer: self.id,
            parents: parent_blks,
        };
        let serialized = &bincode::serialize(&header)?;
        let blk_hash = sha256(&serialized)?;
        let blk = Arc::new(Block { header, txns });

        self.block_pool.insert(blk_hash.clone(), blk.clone());
        for txn_hash in txn_hashs {
            self.utxo_blk_map.insert(txn_hash, blk_hash.clone());
        }

        self.utxo_spent.clear();
        self.new_utxo.clear();

        Ok(blk)
    }

    async fn validate_block(&mut self, block: Arc<Block>) -> Result<Vec<Arc<Block>>, CopycatError> {
        todo!();
    }

    async fn handle_pmaker_msg(&mut self, msg: Arc<Vec<u8>>) -> Result<(), CopycatError> {
        todo!();
    }

    async fn handle_peer_blk_req(
        &mut self,
        peer: NodeId,
        blk_id: Hash,
    ) -> Result<(), CopycatError> {
        if let Some(blk) = self.block_pool.get(&blk_id) {
            self.peer_messenger
                .send(
                    peer,
                    MsgType::NewBlock {
                        blk: blk.as_ref().clone(),
                    },
                )
                .await?
        }
        Ok(())
    }
}
