use super::BlockManagement;
use copycat_protocol::block::{BitcoinBlock, Block};
use copycat_protocol::crypto::{sha256, Hash, PubKey};
use copycat_protocol::transaction::{BitcoinTxn, Txn};
use copycat_protocol::CryptoScheme;
use copycat_utils::CopycatError;

use async_trait::async_trait;
use get_size::GetSize;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::time::Instant;

pub struct BitcoinBlockManagement {
    crypto_scheme: CryptoScheme,
    txn_pool: HashMap<Hash, Arc<Txn>>,
    block_pool: HashMap<Hash, Arc<Block>>,
    utxo: HashSet<(Hash, PubKey)>,
    chain_tail: Hash,
    difficulty: u8, // hash has at most 256 bits
    // fields for constructing new block
    pending_txns: Vec<Hash>,
    block_under_construction: Vec<Arc<Txn>>,
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
            pending_txns: vec![],
            block_under_construction: vec![],
            utxo_spent: HashSet::new(),
            block_emit_time: None,
        }
    }
}

#[async_trait]
impl BlockManagement for BitcoinBlockManagement {
    async fn record_new_txn(&mut self, txn: Arc<Txn>) -> Result<(), CopycatError> {
        let txn_hash = sha256(&bincode::serialize(txn.as_ref())?)?;
        self.txn_pool.insert(txn_hash.clone(), txn);
        self.pending_txns.push(txn_hash);
        Ok(())
    }

    // this method may get canceled in the middle
    async fn prepare_new_block(&mut self) -> Result<(), CopycatError> {
        // let mut modified = false;

        // // got a new chain tail, start over
        // if self.block_under_construction.prev_hash != self.chain_tail {
        //     self.block_emit_time = None;
        //     self.utxo_spent.clear();
        //     self.block_under_construction.txns.clear();
        //     self.block_under_construction.prev_hash = self.chain_tail.clone();
        //     modified = true;
        // }

        // let last_txn_in_block = self.block_under_construction.txns.last();
        // if self.pending_txns.len() > 0
        //     && last_txn_in_block.is_some()
        //     && self
        //         .txn_pool
        //         .get(self.pending_txns.first().unwrap())
        //         .unwrap()
        //         == last_txn_in_block.clone().unwrap()
        // {
        //     // the pending transaction is already in block

        //     // first add the utxos spent to utxo_spent
        //     let (sender, in_utxos) = match last_txn_in_block.unwrap().as_ref() {
        //         Txn::Bitcoin { txn } => match txn {
        //             BitcoinTxn::Send {
        //                 sender, in_utxo, ..
        //             } => (sender, in_utxo),
        //             _ => unreachable!(),
        //         },
        //         _ => unreachable!(),
        //     };
        //     for in_utxo in in_utxos {
        //         self.utxo_spent.insert((in_utxo.clone(), sender.clone()));
        //     }

        //     // then move on
        //     self.pending_txns.remove(0);
        // }

        // // if block is not full yet, try adding new txns
        // loop {
        //     // block is large enough
        //     if self.block_under_construction.get_size() > 0x1000000 {
        //         break;
        //     }
        //     // all transactions are included in block
        //     if self.pending_txns.is_empty() {
        //         break;
        //     }

        //     // check for double spending
        //     let txn_hash = self.pending_txns.first().unwrap();
        //     let txn = self.txn_pool.get(txn_hash).unwrap();
        //     let (sender, in_utxos) = match txn.as_ref() {
        //         Txn::Bitcoin { txn } => match txn {
        //             BitcoinTxn::Send {
        //                 sender, in_utxo, ..
        //             } => (sender, in_utxo),
        //             _ => unreachable!(),
        //         },
        //         _ => unreachable!(),
        //     };

        //     for in_utxo in in_utxos {
        //         let key = (in_utxo.clone(), sender.clone());
        //         if self.utxo.contains(&key) && !self.utxo_spent.contains(&key) {
        //             self.block_under_construction.txns.push(txn.clone());
        //             modified = true;
        //         }
        //     }
        // }

        todo!()
    }

    async fn wait_to_propose(&mut self) -> Result<(), CopycatError> {
        todo!()
    }

    async fn get_new_block(&mut self) -> Result<Arc<Block>, CopycatError> {
        todo!()
    }

    async fn validate_block(&mut self, block: &Block) -> Result<bool, CopycatError> {
        todo!()
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
