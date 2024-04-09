use super::BlockManagement;
use crate::config::BitcoinConfig;
use crate::context::{BlkCtx, TxnCtx};
use crate::peers::PeerMessenger;

use crate::protocol::block::{Block, BlockHeader};
use crate::protocol::crypto::{DummyMerkleTree, Hash, PubKey};
use crate::protocol::transaction::{BitcoinTxn, Txn};
use crate::protocol::{CryptoScheme, MsgType};
use crate::utils::{CopycatError, NodeId};

use async_trait::async_trait;
use get_size::GetSize;
use primitive_types::U256;
use rand::Rng;

use tokio::time::{Duration, Instant};

use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

// sha256 in rust takes ~ 1.9 us for a 80 byte block header
const HEADER_HASH_TIME: f64 = 0.0000019;

pub struct BitcoinBlockManagement {
    id: NodeId,
    crypto_scheme: CryptoScheme,
    txn_pool: HashMap<Hash, (Arc<Txn>, Arc<TxnCtx>)>,
    block_pool: HashMap<Hash, (Arc<Block>, Arc<BlkCtx>, u64)>,
    utxo: HashSet<(Hash, PubKey)>,
    chain_tail: Hash,
    difficulty: u8, // hash has at most 256 bits
    compute_power: f64,
    // fields for constructing new block
    pending_txns: VecDeque<Hash>,
    block_under_construction: Vec<Hash>,
    merkle_root: Option<DummyMerkleTree>,
    block_size: usize,
    new_utxo: HashSet<(Hash, PubKey)>,
    utxo_spent: HashSet<(Hash, PubKey)>,
    block_emit_time: Option<Instant>,
    pow_time: Option<Duration>, // for debugging
    // for requesting missing blocks
    peer_messenger: Arc<PeerMessenger>,
    orphan_blocks: HashMap<Hash, HashSet<Hash>>,
}

impl BitcoinBlockManagement {
    pub fn new(
        id: NodeId,
        crypto_scheme: CryptoScheme,
        config: BitcoinConfig,
        peer_messenger: Arc<PeerMessenger>,
    ) -> Self {
        Self {
            id,
            crypto_scheme,
            txn_pool: HashMap::new(),
            block_pool: HashMap::new(),
            utxo: HashSet::new(),
            chain_tail: U256::zero(),
            difficulty: config.difficulty,
            compute_power: config.compute_power,
            pending_txns: VecDeque::new(),
            block_under_construction: vec![],
            merkle_root: None,
            block_size: 0,
            new_utxo: HashSet::new(),
            utxo_spent: HashSet::new(),
            block_emit_time: None,
            pow_time: None,
            peer_messenger,
            orphan_blocks: HashMap::new(),
        }
    }

    pub fn get_chain_length(&self) -> u64 {
        if self.chain_tail.is_zero() {
            0u64
        } else {
            let (_, _, chain_tail_height) = self.block_pool.get(&self.chain_tail).unwrap();
            *chain_tail_height
        }
    }

    pub fn update_orphans_and_return_tail(&mut self, block_hash: Hash, height: u64) -> (Hash, u64) {
        match self.orphan_blocks.remove(&block_hash) {
            Some(children) => {
                let mut longest_tail = block_hash;
                let mut longest_tail_height = height;
                for child in children {
                    self.block_pool.get_mut(&child).unwrap().2 = height + 1;
                    let (new_decendant, new_decendant_height) =
                        self.update_orphans_and_return_tail(child, height + 1);
                    if new_decendant_height > longest_tail_height {
                        longest_tail = new_decendant;
                        longest_tail_height = new_decendant_height;
                    }
                }
                (longest_tail, longest_tail_height)
            }
            None => (block_hash, height),
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
        let x = u.log(p);
        let pow_time = Duration::from_secs_f64(HEADER_HASH_TIME * x / self.compute_power); // assume parallelism
        pf_trace!(
            self.id;
            "POW takes {} sec; p: {}, u: {}, x: {}, block size: {}, block len: {}",
            pow_time.as_secs_f64(),
            p, u, x,
            self.block_size,
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
                        Some((txn, _)) => match txn.as_ref() {
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
    async fn record_new_txn(
        &mut self,
        txn: Arc<Txn>,
        ctx: Arc<TxnCtx>,
    ) -> Result<bool, CopycatError> {
        let txn_hash = ctx.id;
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

        self.txn_pool.insert(txn_hash.clone(), (txn, ctx));
        self.pending_txns.push_back(txn_hash);
        Ok(true)
    }

    async fn prepare_new_block(&mut self) -> Result<(), CopycatError> {
        let start_time = Instant::now();

        let mut modified = false;
        let mut execution_delay = Duration::from_secs(0);

        // if block is not full yet, try adding new txns
        loop {
            // block is large enough
            if self.block_size > 0x100000 {
                break;
            }

            let txn_hash = match self.pending_txns.pop_front() {
                Some(hash) => hash,
                None => break, // no pending transactions
            };

            // check for double spending
            let (txn, _) = self.txn_pool.get(&txn_hash).unwrap();
            match txn.as_ref() {
                Txn::Bitcoin { txn: bitcoin_txn } => match bitcoin_txn {
                    BitcoinTxn::Send {
                        sender,
                        in_utxo: in_utxos,
                        receiver,
                        out_utxo,
                        remainder,
                        script_bytes,
                        script_runtime,
                        script_succeed,
                        ..
                    } => {
                        let mut valid: bool = true;
                        for in_utxo in in_utxos {
                            let key = (in_utxo.clone(), sender.clone());
                            // if a transaction causes conflict, we only need to keep one of them even if the block is dropped
                            if (self.utxo.contains(&key) || self.new_utxo.contains(&key))
                                && !self.utxo_spent.contains(&key)
                            {
                                // valid transaction
                            } else {
                                valid = false; // double spending occurs here
                                break;
                            }
                        }

                        // execute the script if the txn is valid
                        if valid {
                            execution_delay += *script_runtime;
                            if !script_succeed {
                                valid = false;
                            }
                        }

                        if valid {
                            self.block_under_construction.push(txn_hash.clone());
                            for in_utxo in in_utxos {
                                self.utxo_spent.insert((in_utxo.clone(), sender.clone()));
                            }
                            if *out_utxo > 0 {
                                self.new_utxo.insert((txn_hash.clone(), receiver.clone()));
                            }
                            if *remainder > 0 {
                                self.new_utxo.insert((txn_hash.clone(), sender.clone()));
                            }
                            self.block_size += txn.get_size() + *script_bytes as usize;
                            modified = true;
                        } else {
                            self.pending_txns.push_back(txn_hash); // some dependencies might be missing, retry later
                        }
                    }
                    BitcoinTxn::Grant { receiver, .. } => {
                        // bypass double spending check
                        self.block_under_construction.push(txn_hash.clone());
                        self.new_utxo.insert((txn_hash.clone(), receiver.clone()));
                        self.block_size += txn.get_size();
                        modified = true;
                    }
                    BitcoinTxn::Incentive { .. } => unreachable!(),
                },
                _ => unreachable!(),
            };
        }

        tokio::time::sleep(execution_delay).await;

        if modified || self.block_emit_time.is_none() {
            self.merkle_root =
                Some(DummyMerkleTree::new(self.block_under_construction.len()).await?);
            let start_pow = Instant::now();
            let pow_time = self.get_pow_time();
            self.pow_time = Some(pow_time);
            self.block_emit_time = Some(start_pow + pow_time);
            let runtime = Instant::now() - start_time;
            pf_debug!(self.id; "preparing new block takes {:?}, execution delay is {:?}", runtime, execution_delay);
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

    async fn get_new_block(&mut self) -> Result<(Arc<Block>, Arc<BlkCtx>), CopycatError> {
        // TODO: add incentive txn
        let txns_with_ctx: Vec<(Arc<Txn>, Arc<TxnCtx>)> = self
            .block_under_construction
            .drain(0..)
            .map(|txn_hash| self.txn_pool.get(&txn_hash).unwrap().clone())
            .collect();
        let txns = txns_with_ctx.iter().map(|(txn, _)| txn.clone()).collect();
        let txn_ctxs = txns_with_ctx.into_iter().map(|(_, ctx)| ctx).collect();

        let header = BlockHeader::Bitcoin {
            prev_hash: self.chain_tail.clone(),
            merkle_root: self.merkle_root.clone().unwrap().0, // TODO
            nonce: 1,                                         // use nonce = 1 to indicate valid pow
        };
        let block_ctx = Arc::new(BlkCtx::from_header_and_txns(&header, txn_ctxs)?);
        let hash = block_ctx.id;
        let block = Arc::new(Block { header, txns });

        let chain_length = self.get_chain_length();
        self.block_pool
            .insert(hash, (block.clone(), block_ctx.clone(), chain_length + 1));
        self.chain_tail = hash;

        for utxo in self.new_utxo.drain() {
            self.utxo.insert(utxo);
        }
        for utxo in self.utxo_spent.drain() {
            self.utxo.remove(&utxo);
        }
        pf_debug!(
            self.id;
            "Block {} emitted at {:?}, pow takes {} sec, actual block size is {}+{}={}, block len is {}, block_size is {}, {} pending txns left ({:?})",
            hash,
            self.block_emit_time.unwrap(),
            self.pow_time.unwrap().as_secs_f64(),
            block.header.get_size(),
            block.txns.get_size(),
            block.get_size(),
            block.txns.len(),
            self.block_size,
            self.pending_txns.len(),
            block.header,
        );
        self.block_emit_time = None;
        self.merkle_root = None;
        self.pow_time = None;
        self.block_size = 0;

        Ok((block, block_ctx))
    }

    async fn validate_block(
        &mut self,
        block: Arc<Block>,
        blk_ctx: Arc<BlkCtx>,
    ) -> Result<Vec<(Arc<Block>, Arc<BlkCtx>)>, CopycatError> {
        let block_hash = blk_ctx.id;
        if self.block_pool.contains_key(&block_hash) {
            // duplicate, do nothing
            return Ok(vec![]);
        }

        let (merkle_root, prev_hash, nonce) = match &block.header {
            BlockHeader::Bitcoin {
                merkle_root,
                prev_hash,
                nonce,
                ..
            } => (merkle_root, prev_hash, nonce),
            _ => unreachable!(),
        };

        // hash verification
        tokio::time::sleep(Duration::from_secs_f64(HEADER_HASH_TIME)).await;
        if *nonce != 1 {
            return Ok(vec![]); // invalid POW
        }

        // validate txns
        assert!(block.txns.len() == blk_ctx.txn_ctx.len());
        for idx in 0..block.txns.len() {
            let txn = &block.txns[idx];
            let txn_ctx = &blk_ctx.txn_ctx[idx];
            let txn_hash = txn_ctx.id;
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
                    if !self
                        .crypto_scheme
                        .verify(sender, &serialized_in_utxo, sender_signature)
                        .await?
                    {
                        return Ok(vec![]);
                    }
                }
                if !self.validate_txn(bitcoin_txn)? {
                    return Ok(vec![]);
                }
            };
        }

        // merkle root verification
        let merkle_tree = DummyMerkleTree(merkle_root.clone());
        if !merkle_tree.verify(block.txns.len()).await? {
            return Ok(vec![]);
        }

        // height of block in the chain or 0 if the height is unknown because we have not seen its parent yet
        let blk_height = if prev_hash.is_zero() {
            1u64
        } else {
            match self.block_pool.get(prev_hash) {
                Some((_, _, parent_height)) => {
                    if *parent_height > 0 {
                        parent_height + 1
                    } else {
                        0
                    }
                }
                None => 0,
            }
        };

        self.block_pool
            .insert(block_hash.clone(), (block.clone(), blk_ctx, blk_height));

        if blk_height == 0 {
            // we are missing an ancestor, adding it to orphan blocks
            match self.orphan_blocks.entry(*prev_hash) {
                Occupied(mut entry) => {
                    entry.get_mut().insert(block_hash);
                }
                Vacant(entry) => {
                    entry.insert(HashSet::from([block_hash]));
                }
            }

            // find the missing ancestor and send a request for it
            let mut parent = prev_hash;
            loop {
                assert!(!parent.is_zero());
                if let Some((block, _, height)) = self.block_pool.get(parent) {
                    assert!(*height == 0);
                    parent = match &block.header {
                        BlockHeader::Bitcoin { prev_hash, .. } => prev_hash,
                        _ => unreachable!(),
                    }
                } else {
                    break;
                }
            }

            pf_debug!(self.id; "block {} arrived early, still waiting for its ancestor {}", block_hash, parent);
            // request the missing ancestor from peers
            self.peer_messenger
                .gossip(
                    MsgType::BlockReq { blk_id: *parent },
                    HashSet::from([self.id]),
                )
                .await?;
            return Ok(vec![]);
        }

        // there might be decendants of this block update child heights and use them as new chain tail
        let (new_tail, new_tail_height) =
            self.update_orphans_and_return_tail(block_hash, blk_height);
        let (new_tail_block, new_tail_ctx, _) = self.block_pool.get(&new_tail).unwrap();
        let new_tail_parent = match &new_tail_block.header {
            BlockHeader::Bitcoin { prev_hash, .. } => prev_hash,
            _ => unreachable!(),
        };

        if new_tail != block_hash {
            pf_debug!(self.id; "found block {}'s decendant {}", block_hash, new_tail)
        }

        // the new block is the tail of a shorter chain, do nothing
        let chain_length = self.get_chain_length();
        if new_tail_height <= chain_length {
            pf_debug!(self.id; "new chain is shorter: new chain {} vs old chain {}", new_tail_height, chain_length);
            return Ok(vec![]);
        }

        // find blocks belonging to the diverging chain up to the common ancestor
        let mut new_chain: Vec<(Arc<Block>, Arc<BlkCtx>)> =
            vec![(new_tail_block.clone(), new_tail_ctx.clone())];
        let mut new_tail_ancester = new_tail_parent;
        let mut new_tail_ancester_height = new_tail_height - 1;
        while new_tail_ancester_height > chain_length {
            let (parent, ctx, parent_height) = self.block_pool.get(new_tail_ancester).unwrap();
            assert!(new_tail_ancester_height == *parent_height);
            new_chain.push((parent.clone(), ctx.clone()));
            new_tail_ancester = match &parent.header {
                BlockHeader::Bitcoin { prev_hash, .. } => prev_hash,
                _ => unreachable!(),
            };
            new_tail_ancester_height -= 1;
        }

        // find the old tail
        let mut old_chain: Vec<(Arc<Block>, Arc<BlkCtx>)> = vec![];
        let mut old_tail_ancester = &self.chain_tail;
        loop {
            if new_tail_ancester == old_tail_ancester {
                // found a common ancester, break
                break;
            }
            let (new_tail_parent, new_tail_ctx, new_tail_parent_height) =
                self.block_pool.get(new_tail_ancester).unwrap();
            let (old_tail_parent, old_tail_ctx, old_tail_parent_height) =
                self.block_pool.get(old_tail_ancester).unwrap();
            assert!(new_tail_parent_height == old_tail_parent_height);

            new_chain.push((new_tail_parent.clone(), new_tail_ctx.clone()));
            old_chain.push((old_tail_parent.clone(), old_tail_ctx.clone()));

            new_tail_ancester = match &new_tail_parent.header {
                BlockHeader::Bitcoin { prev_hash, .. } => prev_hash,
                _ => unreachable!(),
            };
            old_tail_ancester = match &old_tail_parent.header {
                BlockHeader::Bitcoin { prev_hash, .. } => prev_hash,
                _ => unreachable!(),
            };
        }

        // undo old chain
        let mut undo_utxo_spent: HashSet<(Hash, PubKey)> = HashSet::new();
        let mut undo_new_utxo: HashSet<(Hash, PubKey)> = HashSet::new();
        let mut undo_txns: HashSet<Hash> = HashSet::new();
        for (undo_blk, undo_blk_ctx) in old_chain {
            // add old tail parent to undo set
            assert!(undo_blk.txns.len() == undo_blk_ctx.txn_ctx.len());
            for idx in (0..undo_blk.txns.len()).rev() {
                let txn = &undo_blk.txns[idx];
                let txn_ctx = &undo_blk_ctx.txn_ctx[idx];
                let txn_hash = txn_ctx.id;
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
        }
        // reverse the new tail so that it goes in order
        new_chain.reverse();

        let mut total_exec_time = Duration::from_secs(0);
        // apply transactions of the new chain and check if this chain is valid
        let mut new_utxos = undo_utxo_spent;
        let mut utxos_spent = undo_new_utxo;
        let mut txns_applied: HashSet<Hash> = HashSet::new();
        for (new_block, new_block_ctx) in new_chain.iter() {
            assert!(new_block.txns.len() == new_block_ctx.txn_ctx.len());
            for idx in 0..new_block.txns.len() {
                let txn = &new_block.txns[idx];
                let txn_ctx = &new_block_ctx.txn_ctx[idx];
                let txn_hash = txn_ctx.id;
                let bitcoin_txn = match txn.as_ref() {
                    Txn::Bitcoin { txn } => txn,
                    _ => unreachable!(),
                };

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
                        } else if new_utxos.contains(&utxo) {
                            // good case, using new utxos created but not committed yet
                            new_utxos.remove(&utxo);
                        } else {
                            // pf_debug!(self.id; "double spending - in utxo ({}): {}, in utxo_spent ({}): {}, in new_utxos ({}): {}", self.utxo.len(), self.utxo.contains(&utxo), utxos_spent.len(), utxos_spent.contains(&utxo), new_utxos.len(), new_utxos.contains(&utxo));
                            tokio::time::sleep(total_exec_time).await;
                            return Ok(vec![]);
                        }
                    }
                }

                // execute the txn script
                if let BitcoinTxn::Send {
                    script_runtime,
                    script_succeed,
                    ..
                } = bitcoin_txn
                {
                    total_exec_time += *script_runtime;
                    if !script_succeed {
                        tokio::time::sleep(total_exec_time).await;
                        return Ok(vec![]);
                    }
                }

                match bitcoin_txn {
                    BitcoinTxn::Send {
                        sender, receiver, ..
                    } => {
                        new_utxos.insert((txn_hash.clone(), sender.clone()));
                        new_utxos.insert((txn_hash.clone(), receiver.clone()));
                    }
                    BitcoinTxn::Grant { receiver, .. } => {
                        new_utxos.insert((txn_hash.clone(), receiver.clone()));
                    }
                    BitcoinTxn::Incentive { receiver, .. } => {
                        new_utxos.insert((txn_hash.clone(), receiver.clone()));
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
        self.merkle_root = None;
        self.pow_time = None;
        self.pending_txns
            .extend(&mut self.block_under_construction.drain(0..));
        self.block_size = 0;
        self.utxo_spent.clear();
        self.new_utxo.clear();
        self.chain_tail = new_tail;

        // add new_utxos and remove utxo_spent from self.utxo
        self.utxo.extend(new_utxos);
        self.utxo.retain(|utxo| !utxos_spent.contains(utxo));

        // add undo_txns back in pending_txns and remove txns_applied from pending_txns
        self.pending_txns.extend(undo_txns);
        self.pending_txns
            .retain(|txn_hash| !txns_applied.contains(txn_hash));

        tokio::time::sleep(total_exec_time).await;
        pf_debug!(self.id; "validation complete, execution takes {:?}", total_exec_time);
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

    async fn handle_peer_blk_req(
        &mut self,
        peer: NodeId,
        blk_id: Hash,
    ) -> Result<(), CopycatError> {
        // only return the req if the block is known
        if let Some((blk, _, _)) = self.block_pool.get(&blk_id) {
            self.peer_messenger
                .send(
                    peer,
                    // MsgType::BlockResp {
                    //     id: blk_id,
                    //     blk: blk.as_ref().clone(),
                    // },
                    MsgType::NewBlock {
                        blk: blk.as_ref().clone(),
                    },
                )
                .await?
        }
        Ok(())
    }

    //     async fn handle_peer_blk_resp(
    //         &mut self,
    //         peer: NodeId,
    //         blk_id: Hash,
    //         block: Arc<Block>,
    //     ) -> Result<(), CopycatError> {
    //         unimplemented!()
    //     }
}

#[cfg(test)]
mod bitcoin_block_management_test {
    use crate::utils::CopycatError;

    #[test]
    fn test_pow() -> Result<(), CopycatError> {
        let p = 1f64 - 2f64.powf(-(10 as f64));
        let u = p.powf(2000f64);
        let mut cdf = 1f64;
        let mut curr = 0u64;
        let x = loop {
            cdf *= p;
            curr += 1;
            if cdf < u {
                break curr;
            }
        };
        if x == 2000 {
            Ok(())
        } else {
            Err(CopycatError(format!("expected x to be 2000, actual: {x}")))
        }
    }
}
