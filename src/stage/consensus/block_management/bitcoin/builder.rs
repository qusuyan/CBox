use super::{BlockManagement, CurBlockState, HEADER_HASH_TIME};
use crate::config::BitcoinPBSConfig;
use crate::context::{BlkCtx, TxnCtx};
use crate::peers::PeerMessenger;
use crate::protocol::block::{Block, BlockHeader};
use crate::protocol::crypto::{DummyMerkleTree, Hash, PubKey};
use crate::protocol::MsgType;
use crate::stage::process_illusion;
use crate::transaction::BitcoinTxn;
use crate::transaction::Txn;
use crate::vcores::VCoreGroup;
use crate::{CopycatError, NodeId};

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use atomic_float::AtomicF64;
use tokio::time::{Duration, Instant};

use async_trait::async_trait;

pub struct BitcoinBuilderBlockManagement {
    id: NodeId,
    delay: Arc<AtomicF64>,
    txn_pool: HashMap<Hash, (Arc<Txn>, Arc<TxnCtx>)>, // txn_id -> txn
    utxo: HashMap<Hash, HashSet<(Hash, PubKey)>>,     // blk_id -> utxos
    blk_pool: HashMap<Hash, (Arc<Block>, Arc<BlkCtx>, HashSet<Hash>, usize)>, // blk_id -> (block, blk_ctx, child blks, blk height)
    // fields for constructing new block
    builder_difficulty: u8,
    core_group: Arc<VCoreGroup>,
    pending_txns: HashMap<Hash, Vec<Hash>>, // blk_id -> pending txns
    prev_hash: Hash,
    block_under_construction: Vec<Hash>,
    merkle_root: Option<DummyMerkleTree>,
    block_size: usize,
    blk_emit_time: Option<Instant>,
    // for peer communication
    peer_messenger: Arc<PeerMessenger>,
    pending_blks: HashMap<Hash, HashSet<Hash>>,
}

impl BitcoinBuilderBlockManagement {
    pub fn new(
        id: NodeId,
        config: BitcoinPBSConfig,
        delay: Arc<AtomicF64>,
        core_group: Arc<VCoreGroup>,
        peer_messenger: Arc<PeerMessenger>,
    ) -> Self {
        Self {
            id,
            builder_difficulty: config.builder_difficulty,
            delay,
            core_group,
            peer_messenger,
            txn_pool: HashMap::new(),
            utxo: HashMap::new(),
            blk_pool: HashMap::new(),
            pending_txns: HashMap::new(),
            prev_hash: Hash::zero(),
            block_under_construction: vec![],
            merkle_root: None,
            block_size: 0,
            blk_emit_time: None,
            pending_blks: HashMap::new(),
        }
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

    fn update_height_for_descendants(&mut self, blk_id: Hash, cur_height: usize) {
        let (_, _, children, height) = self.blk_pool.get_mut(&blk_id).unwrap();
        *height = cur_height;
        for child in children.clone() {
            self.update_height_for_descendants(child, cur_height + 1);
        }
    }

    fn update_utxos_for_descendants(
        &mut self,
        blk_id: &Hash,
        mut base_utxo: HashSet<(Hash, PubKey)>,
    ) -> Duration {
        let mut exec_time = Duration::from_secs(0);
        let (block, blk_ctx, children, _) = self.blk_pool.get_mut(&blk_id).unwrap();

        for idx in 0..block.txns.len() {
            let txn = block.txns[idx].as_ref();
            let txn_ctx = blk_ctx.txn_ctx[idx].as_ref();
            let bitcoin_txn = match txn {
                Txn::Bitcoin { txn } => txn,
                _ => unreachable!(),
            };

            match bitcoin_txn {
                BitcoinTxn::Grant { receiver, .. } => {
                    base_utxo.insert((txn_ctx.id, receiver.clone()));
                }
                BitcoinTxn::Incentive { receiver, .. } => {
                    base_utxo.insert((txn_ctx.id, receiver.clone()));
                }
                BitcoinTxn::Send {
                    sender,
                    in_utxo,
                    receiver,
                    out_utxo,
                    remainder,
                    script_runtime,
                    ..
                } => {
                    for in_tx in in_utxo {
                        base_utxo.remove(&(*in_tx, sender.clone()));
                    }
                    if *out_utxo > 0 {
                        base_utxo.insert((txn_ctx.id, receiver.clone()));
                    }
                    if *remainder > 0 {
                        base_utxo.insert((txn_ctx.id, sender.clone()));
                    }
                    exec_time += *script_runtime;
                }
            }
        }

        for child in children.clone() {
            let child_exec_time = self.update_utxos_for_descendants(&child, base_utxo.clone());
            exec_time += child_exec_time;
        }

        self.utxo.insert(*blk_id, base_utxo);
        exec_time
    }
}

#[async_trait]
impl BlockManagement for BitcoinBuilderBlockManagement {
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
        for pending_txns in self.pending_txns.values_mut() {
            pending_txns.push(txn_hash);
        }
        Ok(true)
    }

    async fn prepare_new_block(&mut self) -> Result<CurBlockState, CopycatError> {
        todo!();
    }

    async fn wait_to_propose(&self) -> Result<(), CopycatError> {
        todo!();
    }

    async fn get_new_block(&mut self) -> Result<(Arc<Block>, Arc<BlkCtx>), CopycatError> {
        todo!();
    }

    async fn validate_block(
        &mut self,
        block: Arc<Block>,
        blk_ctx: Arc<BlkCtx>,
    ) -> Result<Vec<(Arc<Block>, Arc<BlkCtx>)>, CopycatError> {
        let block_hash = blk_ctx.id;
        if self.blk_pool.contains_key(&block_hash) {
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

        let mut verification_timeout = Duration::from_secs_f64(HEADER_HASH_TIME);
        if *nonce != 1 {
            process_illusion(verification_timeout, &self.delay).await;
            return Ok(vec![]); // invalid POW
        }

        // merkle root verification
        let (correct, timeout) = DummyMerkleTree::verify(merkle_root, block.txns.len())?;
        verification_timeout += timeout;
        if !correct {
            process_illusion(verification_timeout, &self.delay).await;
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
            1usize
        } else {
            match self.blk_pool.get_mut(prev_hash) {
                Some((_, _, siblings, parent_height)) => {
                    siblings.insert(block_hash);
                    if *parent_height > 0 {
                        *parent_height + 1
                    } else {
                        0
                    }
                }
                None => {
                    // we are missing an parent, adding it to orphan blocks
                    let siblings = self
                        .pending_blks
                        .entry(*prev_hash)
                        .or_insert(HashSet::new());
                    siblings.insert(block_hash);
                    // request the missing ancestor from peers
                    let mut buf = [0u8; 32];
                    prev_hash.to_little_endian(&mut buf);
                    self.peer_messenger
                        .gossip(
                            MsgType::BlockReq {
                                msg: Vec::from(buf),
                            },
                            HashSet::from([self.id]),
                        )
                        .await?;
                    0
                }
            }
        };

        let children = self
            .pending_blks
            .remove(&block_hash)
            .unwrap_or(HashSet::new());
        self.blk_pool.insert(
            block_hash,
            (block.clone(), blk_ctx.clone(), children.clone(), blk_height),
        );

        if blk_height == 0 {
            // missing some ancestor, do nothing else
            return Ok(vec![(block, blk_ctx)]);
        }

        // update height of all descendants
        for child in children {
            self.update_height_for_descendants(child, blk_height + 1);
        }

        // update utxos for self and descendants
        let base_utxo = self.utxo.get(prev_hash).unwrap();
        let exec_time = self.update_utxos_for_descendants(&block_hash, base_utxo.clone());
        process_illusion(exec_time, &self.delay).await;

        return Ok(vec![(block, blk_ctx)]);
    }

    async fn handle_pmaker_msg(&mut self, msg: Vec<u8>) -> Result<(), CopycatError> {
        todo!();
    }

    async fn handle_peer_blk_req(
        &mut self,
        peer: NodeId,
        msg: Vec<u8>,
    ) -> Result<(), CopycatError> {
        let blk_id = Hash::from_little_endian(&msg);
        if let Some((blk, _, _, _)) = self.blk_pool.get(&blk_id) {
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

    fn report(&mut self) {}
}
