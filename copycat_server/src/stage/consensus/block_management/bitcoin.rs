use super::BlockManagement;
use copycat_protocol::block::{Block, BlockHeader};
use copycat_protocol::crypto::{sha256, Hash, PubKey};
use copycat_protocol::transaction::{BitcoinTxn, Txn};
use copycat_protocol::CryptoScheme;
use copycat_utils::CopycatError;

use async_trait::async_trait;
use get_size::GetSize;
use rand::Rng;

use tokio::time::{Duration, Instant};

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

// sha256 in rust takes ~ 0.045s / 1MB
const HASH_TIME_PER_1MB: f64 = 0.045;

pub struct BitcoinBlockManagement {
    crypto_scheme: CryptoScheme,
    txn_pool: HashMap<Hash, Arc<Txn>>,
    block_pool: HashMap<Hash, (Arc<Block>, u64)>,
    utxo: HashSet<(Hash, PubKey)>,
    chain_tail: Hash,
    difficulty: u8, // hash has at most 256 bits
    // fields for constructing new block
    pending_txns: VecDeque<Hash>,
    block_under_construction: Vec<Hash>,
    utxo_spent: HashSet<(Hash, PubKey)>,
    block_emit_time: Option<Instant>,
}

impl BitcoinBlockManagement {
    pub fn new(crypto_scheme: CryptoScheme) -> Self {
        Self {
            crypto_scheme,
            txn_pool: HashMap::new(),
            block_pool: HashMap::new(),
            utxo: HashSet::new(),
            chain_tail: vec![],
            difficulty: 10,
            pending_txns: VecDeque::new(),
            block_under_construction: vec![],
            utxo_spent: HashSet::new(),
            block_emit_time: None,
        }
    }

    pub fn get_chain_length(&self) -> u64 {
        if self.chain_tail.is_empty() {
            0u64
        } else {
            let (_, chain_tail_height) = self.block_pool.get(&self.chain_tail).unwrap();
            *chain_tail_height
        }
    }
}

impl BitcoinBlockManagement {
    fn get_pow_time(&self) -> Duration {
        // if we randomly pick a nonce,
        // the possibility of success is 2^(- self.difficulty)
        // the code here simulates a timeout with geometric distribution
        let p = 1f64 - 2f64.powf(-(self.difficulty as f64));
        let u = rand::thread_rng().gen::<f64>();
        let mut cdf = 1f64;
        let mut curr = 0u64;
        let x = loop {
            cdf *= p;
            curr += 1;
            if cdf < u {
                break curr;
            }
        };
        let pow_time = Duration::from_secs_f64(
            HASH_TIME_PER_1MB * (self.block_under_construction.get_size() as f64)
                / (0x1000000 as f64)
                * (x as f64),
        );
        log::trace!(
            "POW takes {} sec; p: {p}, u: {u}, x: {x}, block size: {}, block len: {}",
            pow_time.as_secs_f64(),
            self.block_under_construction.get_size(),
            self.block_under_construction.len(),
        );
        pow_time
    }

    fn validate_txn(&self, txn: &BitcoinTxn) -> Result<bool, CopycatError> {
        match txn {
            BitcoinTxn::Send {
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
                            Txn::Bitcoin { txn } => txn,
                            _ => unreachable!(),
                        },
                        None => return Ok(false), // invalid utxo
                    };

                    let value = match utxo {
                        BitcoinTxn::Incentive { out_utxo, receiver } => {
                            if receiver == txn_sender {
                                out_utxo
                            } else {
                                return Ok(false); // utxo does not belong to sender
                            }
                        }
                        BitcoinTxn::Grant { out_utxo, receiver } => {
                            if receiver == txn_sender {
                                out_utxo
                            } else {
                                return Ok(false); // utxo does not belong to sender
                            }
                        }
                        BitcoinTxn::Send {
                            sender,
                            in_utxo: _,
                            receiver,
                            out_utxo,
                            remainder,
                            sender_signature: _,
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
            BitcoinTxn::Grant { .. } => {
                // bypass txn validity checks
            }
            BitcoinTxn::Incentive { .. } => unreachable!(),
        }

        Ok(true)
    }
}

#[async_trait]
impl BlockManagement for BitcoinBlockManagement {
    async fn record_new_txn(&mut self, txn: Arc<Txn>) -> Result<bool, CopycatError> {
        let txn_hash = sha256(&bincode::serialize(txn.as_ref())?)?;
        // ignore duplicate txns
        if self.txn_pool.contains_key(&txn_hash) {
            return Ok(false);
        }

        let bitcoin_txn = match txn.as_ref() {
            Txn::Bitcoin { txn } => txn,
            _ => unreachable!(),
        };

        if !self.validate_txn(bitcoin_txn)? {
            return Ok(false);
        }

        self.txn_pool.insert(txn_hash.clone(), txn.clone());
        self.pending_txns.push_back(txn_hash);
        Ok(true)
    }

    async fn prepare_new_block(&mut self) -> Result<(), CopycatError> {
        let mut modified = false;

        // if block is not full yet, try adding new txns
        loop {
            // block is large enough
            if self.block_under_construction.get_size() > 0x1000000 {
                break;
            }

            let txn_hash = match self.pending_txns.pop_front() {
                Some(hash) => hash,
                None => break, // no pending transactions
            };

            // check for double spending
            let txn = self.txn_pool.get(&txn_hash).unwrap();
            match txn.as_ref() {
                Txn::Bitcoin { txn: bitcoin_txn } => match bitcoin_txn {
                    BitcoinTxn::Send {
                        sender,
                        in_utxo: in_utxos,
                        ..
                    } => {
                        for in_utxo in in_utxos {
                            let key = (in_utxo.clone(), sender.clone());
                            // if a transaction causes conflict, we only need to keep one of them even if the block is dropped
                            if self.utxo.contains(&key) && !self.utxo_spent.contains(&key) {
                                self.block_under_construction.push(txn_hash.clone());
                                modified = true;
                            }
                        }
                    }
                    BitcoinTxn::Grant { .. } => {
                        self.block_under_construction.push(txn_hash.clone());
                        modified = true;
                    }
                    BitcoinTxn::Incentive { .. } => unreachable!(),
                },
                _ => unreachable!(),
            };
        }

        if modified {
            let start_pow = Instant::now();
            let pow_time = self.get_pow_time();
            self.block_emit_time = Some(start_pow + pow_time);
        }

        Ok(())
    }

    async fn wait_to_propose(&self) -> Result<(), CopycatError> {
        loop {
            if let Some(emit_time) = self.block_emit_time {
                tokio::time::sleep_until(emit_time).await;
                return Ok(());
            }
            tokio::task::yield_now().await;
        }
    }

    async fn get_new_block(&mut self) -> Result<Arc<Block>, CopycatError> {
        // TODO: add incentive txn
        let txns = self
            .block_under_construction
            .drain(0..)
            .map(|txn_hash| self.txn_pool.get(&txn_hash).unwrap().clone())
            .collect();
        let block = Arc::new(Block {
            header: BlockHeader::Bitcoin {
                prev_hash: self.chain_tail.clone(),
                merkle_root: sha256(&bincode::serialize(&txns)?)?, // TODO
                nonce: 1, // use nonce = 1 to indicate valid pow
            },
            txns,
        });

        let serialized = &bincode::serialize(&block.header)?;
        let hash = sha256(&serialized)?;
        let chain_length = self.get_chain_length();
        self.block_pool
            .insert(hash.clone(), (block.clone(), chain_length + 1));
        self.chain_tail = hash;

        for utxo in self.utxo_spent.drain() {
            self.utxo.remove(&utxo);
        }
        self.block_emit_time = None;

        Ok(block)
    }

    async fn validate_block(&mut self, block: Arc<Block>) -> Result<Vec<Arc<Block>>, CopycatError> {
        let serialized = bincode::serialize(&block.header)?;
        let block_hash = sha256(&serialized)?;
        if self.block_pool.contains_key(&block_hash) {
            // duplicate, do nothing
            return Ok(vec![]);
        }

        let prev_hash = match &block.header {
            BlockHeader::Bitcoin { prev_hash, .. } => prev_hash,
            _ => unreachable!(),
        };

        let height = if prev_hash.is_empty() {
            1u64
        } else {
            match self.block_pool.get(prev_hash) {
                Some((_, parent_height)) => parent_height + 1,
                None => unimplemented!(
                    "currently does not support validating blocks whose parent was not recorded"
                ),
            }
        };
        self.block_pool
            .insert(block_hash.clone(), (block.clone(), height));

        // the new block is the tail of a shorter chain, do nothing
        let chain_length = self.get_chain_length();
        if height <= chain_length {
            return Ok(vec![]);
        }

        // TODO: find blocks belonging to the diverging chain up to the common ancestor
        let mut new_chain: Vec<Arc<Block>> = vec![block.clone()];
        let mut new_tail_ancester = prev_hash;
        let mut new_tail_ancester_height = height - 1;
        while new_tail_ancester_height > chain_length {
            let (parent, parent_height) = self.block_pool.get(new_tail_ancester).unwrap();
            assert!(new_tail_ancester_height == *parent_height);
            new_chain.push(parent.clone());
            new_tail_ancester = match &parent.header {
                BlockHeader::Bitcoin { prev_hash, .. } => prev_hash,
                _ => unreachable!(),
            };
            new_tail_ancester_height -= 1;
        }

        let mut old_tail_ancester = &self.chain_tail;
        let mut undo_utxo_spent: HashSet<(Hash, PubKey)> = HashSet::new();
        let mut undo_new_utxo: HashSet<(Hash, PubKey)> = HashSet::new();
        let mut undo_txns: HashSet<Hash> = HashSet::new();
        loop {
            if new_tail_ancester == old_tail_ancester {
                // found a common ancester, break
                break;
            }

            let (new_tail_parent, new_tail_parent_height) =
                self.block_pool.get(new_tail_ancester).unwrap();
            let (old_tail_parent, old_tail_parent_height) =
                self.block_pool.get(old_tail_ancester).unwrap();
            assert!(new_tail_parent_height == old_tail_parent_height);
            new_chain.push(new_tail_parent.clone());
            // add old tail parent to undo set
            for txn in old_tail_parent.txns.iter().rev() {
                // undo_txns.push(txn.clone());
                let txn_hash = sha256(&bincode::serialize(txn)?)?;
                match txn.as_ref() {
                    Txn::Bitcoin { txn } => match txn {
                        BitcoinTxn::Send {
                            sender,
                            in_utxo,
                            receiver,
                            ..
                        } => {
                            for utxo in in_utxo {
                                undo_utxo_spent.insert((utxo.clone(), sender.clone()));
                            }
                            let new_utxo_received = (txn_hash.clone(), receiver.clone());
                            if !undo_utxo_spent.remove(&new_utxo_received) {
                                undo_new_utxo.insert(new_utxo_received);
                            }
                            let new_utxo_remainder = (txn_hash.clone(), sender.clone());
                            if !undo_utxo_spent.remove(&new_utxo_remainder) {
                                undo_new_utxo.insert(new_utxo_remainder);
                            }
                            undo_txns.insert(txn_hash);
                        }
                        BitcoinTxn::Grant { receiver, .. } => {
                            let new_utxo = (txn_hash.clone(), receiver.clone());
                            if !undo_utxo_spent.remove(&new_utxo) {
                                undo_new_utxo.insert(new_utxo);
                            }
                            undo_txns.insert(txn_hash);
                        }
                        BitcoinTxn::Incentive { receiver, .. } => {
                            let new_utxo = (txn_hash, receiver.clone());
                            if !undo_utxo_spent.remove(&new_utxo) {
                                undo_new_utxo.insert(new_utxo);
                            }
                            // incentive txns should not be returned back to pending txns
                        }
                    },
                    _ => unreachable!(),
                }
            }
            new_tail_ancester = match &new_tail_parent.header {
                BlockHeader::Bitcoin { prev_hash, .. } => prev_hash,
                _ => unreachable!(),
            };
            old_tail_ancester = match &old_tail_parent.header {
                BlockHeader::Bitcoin { prev_hash, .. } => prev_hash,
                _ => unreachable!(),
            };
        }
        // reverse the new tail so that it goes in order
        new_chain.reverse();

        // apply transactions of the new chain and check if this chain is valid
        let mut new_utxos = undo_utxo_spent;
        let mut utxos_spent = undo_new_utxo;
        let mut txns_applied: HashSet<Hash> = HashSet::new();
        for new_block in new_chain.iter() {
            let nonce = match new_block.header {
                BlockHeader::Bitcoin { nonce, .. } => nonce,
                _ => unreachable!(),
            };
            // hash verification
            tokio::time::sleep(Duration::from_secs_f64(
                HASH_TIME_PER_1MB * new_block.get_size() as f64 / 0x1000000 as f64,
            ))
            .await;
            if nonce != 1 {
                return Ok(vec![]); // invalid POW
            }

            // TODO: add merkle tree proof

            // TODO: check if all transactions are valid and if double spending occurs
            for txn in new_block.txns.iter() {
                let txn_hash = sha256(&bincode::serialize(txn)?)?;
                let bitcoin_txn = match txn.as_ref() {
                    Txn::Bitcoin { txn } => txn,
                    _ => unreachable!(),
                };

                // validate transaction
                if !self.txn_pool.contains_key(&txn_hash) {
                    if let BitcoinTxn::Send {
                        sender,
                        in_utxo,
                        sender_signature,
                        ..
                    } = bitcoin_txn
                    {
                        let serialized_in_utxo = bincode::serialize(in_utxo)?;
                        if !self.crypto_scheme.verify(
                            sender,
                            &serialized_in_utxo,
                            sender_signature,
                        )? {
                            return Ok(vec![]);
                        }
                    }
                    if !self.validate_txn(bitcoin_txn)? {
                        return Ok(vec![]);
                    }
                    self.txn_pool.insert(txn_hash.clone(), txn.clone());
                }

                // check for double spending
                if let BitcoinTxn::Send {
                    sender, in_utxo, ..
                } = bitcoin_txn
                {
                    for in_txn_hash in in_utxo {
                        let utxo = (in_txn_hash.clone(), sender.clone());
                        if self.utxo.contains(&utxo) && !utxos_spent.contains(&utxo) {
                            // good case, using existing utxo that has not been spent
                            utxos_spent.insert(utxo);
                        } else {
                            if new_utxos.contains(&utxo) {
                                // good case, using new utxos created but not committed yet
                                new_utxos.remove(&utxo);
                            } else {
                                return Ok(vec![]);
                            }
                        }
                    }
                }

                // add to txns_applied so that they will be removed from pending_txns
                if !matches!(bitcoin_txn, BitcoinTxn::Incentive { .. }) {
                    if !undo_txns.remove(&txn_hash) {
                        txns_applied.insert(txn_hash);
                    }
                }
            }
        }

        // the new block is the tail of the longer chain, move over
        self.block_emit_time = None;
        self.pending_txns
            .append(&mut self.block_under_construction.drain(0..).collect());
        self.utxo_spent.clear();
        self.chain_tail = block_hash;

        // add new_utxos and remove utxo_spent from self.utxo
        self.utxo.extend(new_utxos);
        self.utxo.retain(|utxo| !utxos_spent.contains(utxo));

        // add undo_txns back in pending_txns and remove txns_applied from pending_txns
        self.pending_txns.extend(undo_txns);
        self.pending_txns
            .retain(|txn_hash| !txns_applied.contains(txn_hash));

        Ok(new_chain)
    }

    async fn handle_pmaker_msg(&mut self, msg: Arc<Vec<u8>>) -> Result<(), CopycatError> {
        if msg.len() == 0 {
            return Err(CopycatError(String::from(
                "got empty message from pacemaker",
            )));
        }
        self.difficulty = msg[0];
        Ok(())
    }
}
