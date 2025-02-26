use super::{BlockManagement, CurBlockState, HEADER_HASH_TIME};
use crate::config::BitcoinBasicConfig;
use crate::context::{BlkCtx, TxnCtx};
use crate::peers::PeerMessenger;
use crate::protocol::block::{Block, BlockHeader};
use crate::protocol::crypto::vector_snark::DummyMerkleTree;
use crate::protocol::crypto::{Hash, PubKey};
use crate::protocol::transaction::{BitcoinTxn, Txn};
use crate::protocol::MsgType;
use crate::stage::process_illusion;
use crate::utils::{CopycatError, NodeId};
use crate::vcores::VCoreGroup;

use async_trait::async_trait;
use atomic_float::AtomicF64;
use get_size::GetSize;
use primitive_types::U256;
use rand::Rng;

use tokio::time::{Duration, Instant};

use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

pub struct BitcoinBlockManagement {
    id: NodeId,
    delay: Arc<AtomicF64>,
    core_group: Arc<VCoreGroup>,
    txn_pool: HashMap<Hash, (Arc<Txn>, Arc<TxnCtx>)>,
    block_pool: HashMap<Hash, (Arc<Block>, Arc<BlkCtx>, u64)>,
    utxo: HashSet<(Hash, PubKey)>,
    chain_tail: Hash,
    difficulty: u8, // hash has at most 256 bits
    // fields for constructing new block
    pending_txns: VecDeque<Hash>,
    block_under_construction: Vec<Hash>,
    merkle_root: Option<DummyMerkleTree>,
    block_size: usize,
    new_utxo: HashSet<(Hash, PubKey)>,
    utxo_spent: HashSet<(Hash, PubKey)>,
    pow_time: Option<(Instant, f64)>, // for debugging
    // for requesting missing blocks
    peer_messenger: Arc<PeerMessenger>,
    orphan_blocks: HashMap<Hash, HashSet<Hash>>,
}

impl BitcoinBlockManagement {
    pub fn new(
        id: NodeId,
        config: BitcoinBasicConfig,
        delay: Arc<AtomicF64>,
        core_group: Arc<VCoreGroup>,
        peer_messenger: Arc<PeerMessenger>,
    ) -> Self {
        Self {
            id,
            delay,
            core_group,
            txn_pool: HashMap::new(),
            block_pool: HashMap::new(),
            utxo: HashSet::new(),
            chain_tail: U256::zero(),
            difficulty: config.difficulty,
            pending_txns: VecDeque::new(),
            block_under_construction: vec![],
            merkle_root: None,
            block_size: 0,
            new_utxo: HashSet::new(),
            utxo_spent: HashSet::new(),
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
    fn get_pow_time_secs(&self) -> f64 {
        // if we randomly pick a nonce,
        // the possibility of success is 2^(- self.difficulty)
        // the code here simulates a timeout with geometric distribution
        let p = 1f64 - 2f64.powf(-(self.difficulty as f64));
        let u = rand::thread_rng().gen::<f64>();
        let x = u.log(p);
        let pow_time = HEADER_HASH_TIME * x;
        pf_debug!(
            self.id;
            "POW takes {} sec; p: {}, u: {}, x: {}, block size: {}, block len: {}",
            pow_time,
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

        self.txn_pool.insert(txn_hash.clone(), (txn, ctx));
        self.pending_txns.push_back(txn_hash);
        Ok(true)
    }

    async fn prepare_new_block(&mut self) -> Result<CurBlockState, CopycatError> {
        // let start_time = Instant::now();

        let mut modified = false;
        let mut execution_delay = Duration::from_secs(0);

        // if block is not full yet, try adding new txns
        let blk_full = loop {
            // block is large enough
            if self.block_size > 0x100000 {
                break CurBlockState::Full;
            }

            let txn_hash = match self.pending_txns.pop_front() {
                Some(hash) => hash,
                None => break CurBlockState::EmptyMempool, // no pending transactions
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
                            self.block_size += txn.get_size();
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
        };

        if modified || self.pow_time.is_none() {
            let (merkle_root, timeout) = DummyMerkleTree::new(self.block_under_construction.len())?;
            execution_delay += timeout;
            self.merkle_root = Some(merkle_root);
            process_illusion(execution_delay, &self.delay).await;
            let pow_start_time = Instant::now();
            let pow_time = self.get_pow_time_secs();
            self.pow_time = Some((pow_start_time, pow_time));
            // let runtime = Instant::now() - start_time;
            // pf_debug!(self.id; "preparing new block takes {:?}, execution delay is {:?}", runtime, execution_delay);
        } else {
            process_illusion(execution_delay, &self.delay).await;
        }

        Ok(blk_full)
    }

    async fn wait_to_propose(&self) -> Result<(), CopycatError> {
        loop {
            if let Some((pow_start_time, pow_time)) = self.pow_time {
                let compute_power = self.core_group.get_unused();
                let pow_dur = Duration::from_secs_f64(pow_time / compute_power - 0.001);
                let now = Instant::now();
                if now > pow_start_time + pow_dur {
                    return Ok(());
                }
                let pow_remain = pow_start_time + pow_dur - now;
                let recheck_time = std::cmp::min(pow_remain, Duration::from_secs(10));
                tokio::time::sleep(recheck_time).await;
            }
            tokio::task::yield_now().await;
        }
    }

    async fn get_new_block(&mut self) -> Result<(Arc<Block>, Arc<BlkCtx>), CopycatError> {
        // TODO: add incentive txn
        let txns_with_ctx = self
            .block_under_construction
            .drain(0..)
            .map(|txn_hash| self.txn_pool.get(&txn_hash).unwrap().clone());
        let (txns, txn_ctxs) = txns_with_ctx.unzip();

        let header = BlockHeader::Bitcoin {
            proposer: self.id,
            prev_hash: self.chain_tail.clone(),
            merkle_root: self.merkle_root.clone().unwrap().get_root(), // TODO
            nonce: 1, // use nonce = 1 to indicate valid pow
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
        let (pow_start_time, pow_time) = self.pow_time.unwrap();
        pf_info!(
            self.id;
            "Block {} at height {}, pow actually takes {} sec, pow takes {} sec, compute power is {}, actual block size is {}+{}={}, block len is {}, block_size is {}, {} pending txns left ({:?})",
            hash,
            chain_length + 1,
            (Instant::now() - pow_start_time).as_secs_f64(),
            pow_time,
            self.core_group.get_unused(),
            block.header.get_size(),
            block.txns.get_size(),
            block.get_size(),
            block.txns.len(),
            self.block_size,
            self.pending_txns.len(),
            block.header,
        );
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
        let mut verification_timeout = Duration::from_secs_f64(HEADER_HASH_TIME);
        if *nonce != 1 {
            process_illusion(verification_timeout, &self.delay).await;
            return Ok(vec![]); // invalid POW
        }

        // merkle root verification
        let (correct, timeout) = DummyMerkleTree::verify(merkle_root, block.txns.len())?;
        verification_timeout += timeout;
        if !correct {
            return Ok(vec![]);
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
                if !self.validate_txn(bitcoin_txn)? {
                    process_illusion(verification_timeout, &self.delay).await;
                    return Ok(vec![]);
                }
                self.txn_pool
                    .insert(txn_hash, (txn.clone(), txn_ctx.clone()));
            };
        }

        process_illusion(verification_timeout, &self.delay).await;

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
            let mut buf = [0u8; 32];
            parent.to_little_endian(&mut buf);
            self.peer_messenger
                .gossip(
                    MsgType::BlockReq {
                        msg: Vec::from(buf),
                    },
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
            pf_debug!(self.id; "new chain tail {} is shorter: new chain {} vs old chain {}", new_tail_ctx.id, new_tail_height, chain_length);
            return Ok(vec![]);
        }

        pf_debug!(self.id; "moving to a longer chain tail {} at height {}", new_tail_ctx.id, new_tail_height);

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
                            out_utxo,
                            remainder,
                            ..
                        } => {
                            for utxo in in_utxo {
                                undo_utxo_spent.insert((utxo.clone(), sender.clone()));
                            }
                            if *out_utxo > 0 {
                                let new_utxo_received = (txn_hash.clone(), receiver.clone());
                                if !undo_utxo_spent.remove(&new_utxo_received) {
                                    undo_new_utxo.insert(new_utxo_received);
                                }
                            }
                            if *remainder > 0 {
                                let new_utxo_remainder = (txn_hash.clone(), sender.clone());
                                if !undo_utxo_spent.remove(&new_utxo_remainder) {
                                    undo_new_utxo.insert(new_utxo_remainder);
                                }
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
                        if new_utxos.contains(&utxo) {
                            // good case, using new utxos created but not committed yet
                            new_utxos.remove(&utxo);
                        } else if self.utxo.contains(&utxo) && !utxos_spent.contains(&utxo) {
                            // good case, using existing utxo that has not been spent
                            utxos_spent.insert(utxo);
                        } else {
                            // pf_debug!(self.id; "double spending - in utxo ({}): {}, in utxo_spent ({}): {}, in new_utxos ({}): {}", self.utxo.len(), self.utxo.contains(&utxo), utxos_spent.len(), utxos_spent.contains(&utxo), new_utxos.len(), new_utxos.contains(&utxo));
                            tokio::time::sleep(total_exec_time).await;
                            pf_debug!(self.id; "double spending detected with block {}", new_tail_ctx.id);
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
                        pf_debug!(self.id; "script exec failed with block {}", new_tail_ctx.id);
                        return Ok(vec![]);
                    }
                }

                match bitcoin_txn {
                    BitcoinTxn::Send {
                        sender,
                        receiver,
                        out_utxo,
                        remainder,
                        ..
                    } => {
                        if *out_utxo > 0 {
                            let utxo = (txn_hash.clone(), receiver.clone());
                            if !utxos_spent.remove(&utxo) {
                                new_utxos.insert(utxo);
                            }
                        }
                        if *remainder > 0 {
                            let utxo = (txn_hash.clone(), sender.clone());
                            if !utxos_spent.remove(&utxo) {
                                new_utxos.insert(utxo);
                            }
                        }
                    }
                    BitcoinTxn::Grant { receiver, .. } => {
                        let utxo = (txn_hash.clone(), receiver.clone());
                        if !utxos_spent.remove(&utxo) {
                            new_utxos.insert(utxo);
                        }
                    }
                    BitcoinTxn::Incentive { receiver, .. } => {
                        let utxo = (txn_hash.clone(), receiver.clone());
                        if !utxos_spent.remove(&utxo) {
                            new_utxos.insert(utxo);
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

    async fn handle_pmaker_msg(&mut self, msg: Vec<u8>) -> Result<(), CopycatError> {
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
        msg: Vec<u8>,
    ) -> Result<(), CopycatError> {
        let blk_id = U256::from_little_endian(&msg);
        if let Some((blk, _, _)) = self.block_pool.get(&blk_id) {
            // return the req if the block is known
            self.peer_messenger
                .send(peer, MsgType::NewBlock { blk: blk.clone() })
                .await?
        } else {
            // otherwise gossip to other neighbors
            self.peer_messenger
                .gossip(MsgType::BlockReq { msg }, HashSet::from([self.id, peer]))
                .await?;
        }
        Ok(())
    }

    fn report(&mut self) {
        pf_info!(self.id; "PoW compute: {}", self.core_group.get_unused());
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
    use std::{sync::Arc, time::Duration};

    use atomic_float::AtomicF64;

    use crate::{
        config::BitcoinBasicConfig,
        context::BlkCtx,
        peers::PeerMessenger,
        protocol::{
            block::{Block, BlockHeader},
            crypto::Hash,
        },
        stage::consensus::block_management::{
            bitcoin::basic::BitcoinBlockManagement, BlockManagement,
        },
        transaction::{BitcoinTxn, Txn},
        utils::CopycatError,
        vcores::VCoreGroup,
    };

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

    #[tokio::test]
    async fn test_block_validation() -> Result<(), CopycatError> {
        let txn1_1 = Arc::new(Txn::Bitcoin {
            txn: BitcoinTxn::Grant {
                out_utxo: 10,
                receiver: vec![1],
            },
        });
        let txn1_1_id = txn1_1.compute_id()?;

        let txn1_2 = Arc::new(Txn::Bitcoin {
            txn: BitcoinTxn::Grant {
                out_utxo: 10,
                receiver: vec![2],
            },
        });
        let txn1_2_id = txn1_2.compute_id()?;

        let txn1_3 = Arc::new(Txn::Bitcoin {
            txn: BitcoinTxn::Send {
                sender: vec![1],
                in_utxo: vec![txn1_1_id],
                receiver: vec![2],
                out_utxo: 10,
                remainder: 0,
                sender_signature: vec![],
                script_bytes: 400,
                script_runtime: Duration::from_millis(1),
                script_succeed: true,
            },
        });
        let txn1_3_id = txn1_3.compute_id()?;

        let blk1 = Arc::new(Block {
            header: BlockHeader::Bitcoin {
                proposer: 0,
                prev_hash: Hash::zero(),
                merkle_root: Hash::zero(),
                nonce: 1,
            },
            txns: vec![txn1_1.clone(), txn1_2.clone(), txn1_3.clone()],
        });
        let blk1_ctx = Arc::new(BlkCtx::from_blk(&blk1)?);

        // let txn2_1 = Arc::new(Txn::Bitcoin {
        //     txn: BitcoinTxn::Grant {
        //         out_utxo: 10,
        //         receiver: Box::new([3]),
        //     },
        // });
        // let txn2_1_id = txn2_1.compute_id()?;

        // let txn2_2 = Arc::new(Txn::Bitcoin {
        //     txn: BitcoinTxn::Grant {
        //         out_utxo: 10,
        //         receiver: Box::new([4]),
        //     },
        // });
        // let txn2_2_id = txn2_2.compute_id()?;

        // let txn2_3 = Arc::new(Txn::Bitcoin {
        //     txn: BitcoinTxn::Send {
        //         sender: Box::new([3]),
        //         in_utxo: vec![txn2_1_id],
        //         receiver: Box::new([4]),
        //         out_utxo: 10,
        //         remainder: 0,
        //         sender_signature: vec![],
        //         script_bytes: 400,
        //         script_runtime: Duration::from_millis(1),
        //         script_succeed: true,
        //     },
        // });
        // let txn2_3_id = txn2_3.compute_id()?;

        let blk2 = Arc::new(Block {
            header: BlockHeader::Bitcoin {
                proposer: 0,
                prev_hash: Hash::zero(),
                merkle_root: Hash::one(),
                nonce: 1,
            },
            txns: vec![txn1_1, txn1_2, txn1_3],
        });
        let blk2_ctx = Arc::new(BlkCtx::from_blk(&blk2)?);

        let txn3_1 = Arc::new(Txn::Bitcoin {
            txn: BitcoinTxn::Grant {
                out_utxo: 10,
                receiver: vec![5],
            },
        });
        let txn3_1_id = txn3_1.compute_id()?;

        let txn3_2 = Arc::new(Txn::Bitcoin {
            txn: BitcoinTxn::Send {
                sender: vec![2],
                in_utxo: vec![txn1_2_id],
                receiver: vec![3],
                out_utxo: 10,
                remainder: 0,
                sender_signature: vec![],
                script_bytes: 400,
                script_runtime: Duration::from_millis(1),
                script_succeed: true,
            },
        });
        let txn3_2_id = txn3_2.compute_id()?;

        let txn3_3 = Arc::new(Txn::Bitcoin {
            txn: BitcoinTxn::Send {
                sender: vec![5],
                in_utxo: vec![txn3_1_id],
                receiver: vec![4],
                out_utxo: 10,
                remainder: 0,
                sender_signature: vec![],
                script_bytes: 400,
                script_runtime: Duration::from_millis(1),
                script_succeed: true,
            },
        });
        let txn3_3_id = txn3_3.compute_id()?;

        let blk3 = Arc::new(Block {
            header: BlockHeader::Bitcoin {
                proposer: 0,
                prev_hash: blk2_ctx.id,
                merkle_root: Hash::zero(),
                nonce: 1,
            },
            txns: vec![txn3_1, txn3_2, txn3_3],
        });
        let blk3_ctx = Arc::new(BlkCtx::from_blk(&blk3)?);

        let txn4_1 = Arc::new(Txn::Bitcoin {
            txn: BitcoinTxn::Send {
                sender: vec![2],
                in_utxo: vec![txn1_3_id],
                receiver: vec![5],
                out_utxo: 10,
                remainder: 0,
                sender_signature: vec![],
                script_bytes: 400,
                script_runtime: Duration::from_millis(1),
                script_succeed: true,
            },
        });

        let txn4_2 = Arc::new(Txn::Bitcoin {
            txn: BitcoinTxn::Send {
                sender: vec![3],
                in_utxo: vec![txn3_2_id],
                receiver: vec![5],
                out_utxo: 10,
                remainder: 0,
                sender_signature: vec![],
                script_bytes: 400,
                script_runtime: Duration::from_millis(1),
                script_succeed: true,
            },
        });

        let txn4_3 = Arc::new(Txn::Bitcoin {
            txn: BitcoinTxn::Send {
                sender: vec![4],
                in_utxo: vec![txn3_3_id],
                receiver: vec![3],
                out_utxo: 10,
                remainder: 0,
                sender_signature: vec![],
                script_bytes: 400,
                script_runtime: Duration::from_millis(1),
                script_succeed: true,
            },
        });

        let blk4 = Arc::new(Block {
            header: BlockHeader::Bitcoin {
                proposer: 0,
                prev_hash: blk3_ctx.id,
                merkle_root: Hash::zero(),
                nonce: 1,
            },
            txns: vec![txn4_1, txn4_2, txn4_3],
        });
        let blk4_ctx = Arc::new(BlkCtx::from_blk(&blk4)?);

        let mut stage = BitcoinBlockManagement::new(
            1,
            BitcoinBasicConfig::default(),
            Arc::new(AtomicF64::new(0f64)),
            Arc::new(VCoreGroup::new(1)),
            Arc::new(PeerMessenger::new_stub()),
        );

        println!("Validating block 1: {}", blk1_ctx.id);
        let result1 = stage.validate_block(blk1, blk1_ctx).await?;
        assert!(result1.len() == 1);

        println!("Validating block 2: {}", blk2_ctx.id);
        let result2 = stage.validate_block(blk2, blk2_ctx).await?;
        assert!(result2.len() == 0);

        println!("Validating block 3: {}", blk3_ctx.id);
        let result3 = stage.validate_block(blk3, blk3_ctx).await?;
        assert!(result3.len() == 2);

        println!("Validating block 4: {}", blk4_ctx.id);
        let result4 = stage.validate_block(blk4, blk4_ctx).await?;
        assert!(result4.len() == 1);

        Ok(())
    }
}
