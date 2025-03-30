use super::{BlockManagement, CurBlockState};
use crate::config::AptosDiemConfig;
use crate::context::{BlkCtx, TxnCtx};
use crate::protocol::block::{Block, BlockHeader};
use crate::protocol::crypto::signature::P2PSignature;
use crate::protocol::crypto::threshold_signature::ThresholdSignature;
use crate::protocol::crypto::vector_snark::DummyMerkleTree;
use crate::protocol::crypto::{Hash, PrivKey, PubKey};
use crate::protocol::types::aptos::CoA;
use crate::stage::DelayPool;
use crate::transaction::Txn;
use crate::{CopycatError, NodeId, SignatureScheme};

use async_trait::async_trait;
use get_size::GetSize;

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use tokio::sync::Notify;
use tokio::time::Duration;
use tokio::time::Instant;

pub struct AptosBlockManagement {
    id: NodeId,
    blk_size: usize,
    batch_timeout: Duration,
    txn_pool: HashMap<Hash, (Arc<Txn>, Arc<TxnCtx>)>,
    blks_seen: HashSet<(NodeId, u64)>,
    all_nodes: Vec<NodeId>,
    self_idx: usize,
    // mempool
    pending_txns: VecDeque<Hash>,
    pending_txns_pool: HashSet<Hash>,
    // consensus
    round: u64,
    coa_lists: HashMap<u64, Vec<CoA>>,
    num_faulty: usize,
    // building blocks
    block_under_construction: Vec<Arc<Txn>>,
    block_under_construction_ctx: Vec<Arc<TxnCtx>>,
    block_merkle: DummyMerkleTree,
    cur_block_size: usize,
    cur_block_timeout: Option<Instant>,
    // p2p communication
    signature_scheme: SignatureScheme,
    peer_pks: HashMap<NodeId, PubKey>,
    sk: PrivKey,
    threshold_signature: Arc<dyn ThresholdSignature>,
    //
    delay: Arc<DelayPool>,
    _notify: Notify,
}

impl AptosBlockManagement {
    pub fn new(
        id: NodeId,
        p2p_signature: P2PSignature,
        threshold_signature: Arc<dyn ThresholdSignature>,
        config: AptosDiemConfig,
        delay: Arc<DelayPool>,
    ) -> Self {
        let (signature_scheme, peer_pks, sk) = p2p_signature;

        let mut all_nodes: Vec<NodeId> = peer_pks.keys().cloned().collect();
        all_nodes.sort();
        let self_idx = all_nodes
            .iter()
            .position(|node_id| *node_id == id)
            .expect("Missing self");
        let num_faulty = (all_nodes.len() - 1) / 3;

        let coa_lists = HashMap::from([(0, vec![])]);

        Self {
            id,
            blk_size: config.narwhal_blk_size,
            batch_timeout: Duration::from_secs_f64(config.diem_batching_timeout_secs),
            txn_pool: HashMap::new(),
            blks_seen: HashSet::new(),
            all_nodes,
            self_idx,
            pending_txns: VecDeque::new(),
            pending_txns_pool: HashSet::new(),
            round: 0,
            coa_lists,
            num_faulty,
            block_under_construction: vec![],
            block_under_construction_ctx: vec![],
            block_merkle: DummyMerkleTree::new(),
            cur_block_size: 0,
            cur_block_timeout: None,
            signature_scheme,
            peer_pks,
            sk,
            threshold_signature,
            delay,
            _notify: Notify::new(),
        }
    }
}

#[async_trait]
impl BlockManagement for AptosBlockManagement {
    async fn record_new_txn(
        &mut self,
        txn: Arc<Txn>,
        ctx: Arc<TxnCtx>,
    ) -> Result<bool, CopycatError> {
        let txn_id = ctx.id;
        if self.txn_pool.contains_key(&txn_id) {
            return Ok(false);
        }

        self.txn_pool.insert(txn_id.clone(), (txn, ctx));
        if txn_id.0 % self.all_nodes.len() == self.self_idx.into() {
            // TODO: add other txns to the pending queue to avoid censorship
            self.pending_txns.push_back(txn_id);
        }
        self.pending_txns_pool.insert(txn_id);
        Ok(true)
    }

    async fn prepare_new_block(&mut self) -> Result<CurBlockState, CopycatError> {
        let mut txns_inserted = 0;
        let state = loop {
            let next_txn_id = match self.pending_txns.pop_front() {
                Some(txn_id) => txn_id,
                None => break CurBlockState::EmptyMempool,
            };

            if self.pending_txns_pool.remove(&next_txn_id) == false {
                continue;
            }

            let (next_txn, next_txn_ctx) = self.txn_pool.get(&next_txn_id).unwrap();
            let txn_size = GetSize::get_size(next_txn.as_ref());

            self.block_under_construction.push(next_txn.clone());
            self.block_under_construction_ctx.push(next_txn_ctx.clone());
            if self.cur_block_timeout.is_none() {
                self.cur_block_timeout = Some(Instant::now() + self.batch_timeout);
            }
            txns_inserted += 1;
            self.cur_block_size += txn_size;
            if self.cur_block_size >= self.blk_size {
                break CurBlockState::Full;
            }
        };

        let dur = self.block_merkle.append(txns_inserted)?;
        self.delay.process_illusion(dur).await;
        Ok(state)
    }

    async fn wait_to_propose(&self) -> Result<(), CopycatError> {
        loop {
            if self.coa_lists.contains_key(&self.round) {
                if self.cur_block_size >= self.blk_size {
                    return Ok(());
                }
                if let Some(timeout) = self.cur_block_timeout {
                    tokio::time::sleep_until(timeout).await;
                    return Ok(());
                }
            }
            self._notify.notified().await;
        }
    }

    async fn get_new_block(&mut self) -> Result<(Arc<Block>, Arc<BlkCtx>), CopycatError> {
        let txns = std::mem::replace(&mut self.block_under_construction, vec![]);
        let txn_ctx = std::mem::replace(&mut self.block_under_construction_ctx, vec![]);

        let certificates = self.coa_lists.remove(&self.round).unwrap();
        self.round += 1;
        let merkle_root = self.block_merkle.get_root();

        let header_content = (&self.id, &self.round, &certificates, &merkle_root);
        let serialized = bincode::serialize(&header_content)?;
        let (signature, dur) = self.signature_scheme.sign(&self.sk, &serialized)?;
        self.delay.process_illusion(dur).await;

        let header = BlockHeader::Aptos {
            sender: self.id,
            round: self.round,
            certificates,
            merkle_root,
            signature,
        };
        let blk_ctx = Arc::new(BlkCtx::from_header_and_txns(&header, txn_ctx)?);
        let block = Arc::new(Block { header, txns });

        self.blks_seen.insert((self.id, self.round));

        self.cur_block_size = 0;
        self.cur_block_timeout = None;
        self.block_merkle = DummyMerkleTree::new();

        Ok((block, blk_ctx))
    }

    async fn validate_block(
        &mut self,
        _src: NodeId,
        block: Arc<Block>,
        ctx: Arc<BlkCtx>,
    ) -> Result<Vec<(Arc<Block>, Arc<BlkCtx>)>, CopycatError> {
        let Block { header, .. } = block.as_ref();
        let (sender, round, certificates, merkle_root, signature) = match header {
            BlockHeader::Aptos {
                sender,
                round,
                certificates,
                merkle_root,
                signature,
            } => (sender, round, certificates, merkle_root, signature),
            _ => unreachable!(),
        };

        // verify signature
        let sender_pk = match self.peer_pks.get(sender) {
            Some(pk) => pk,
            None => return Ok(vec![]), // unknown sender
        };
        let content = (sender, round, certificates, merkle_root);
        let serialized = bincode::serialize(&content)?;
        let (valid, dur) = self
            .signature_scheme
            .verify(sender_pk, &serialized, signature)?;
        self.delay.process_illusion(dur).await;
        if !valid {
            return Ok(vec![]);
        }

        // either genesis or contains N-f certificates from round r-1
        if *round != 1 {
            let mut valid_needed = self.all_nodes.len() - self.num_faulty;
            for (idx, certificate) in certificates.iter().enumerate() {
                if certificates.len() - idx < valid_needed {
                    // not enough certificates left, kill early
                    return Ok(vec![]);
                }
                if certificate.round != round - 1 {
                    continue;
                }
                let (valid, dur) = certificate.validate(self.threshold_signature.as_ref())?;
                self.delay.process_illusion(dur).await;
                if !valid {
                    continue;
                }
                valid_needed -= 1;
                if valid_needed == 0 {
                    break;
                }
            }

            if valid_needed > 0 {
                // not enough certificates
                return Ok(vec![]);
            }
        }

        self.blks_seen.insert((*sender, *round));

        Ok(vec![(block, ctx)])
    }

    async fn handle_pmaker_msg(&mut self, msg: Vec<u8>) -> Result<(), CopycatError> {
        let coa_list: Vec<CoA> = bincode::deserialize(&msg)?;
        let round = coa_list.first().unwrap().round;
        pf_debug!(self.id; "got coa list for round {}, current round is {} (curblock len: {}, curblock size: {}, curblock timeout: {:?})", round, self.round, self.block_under_construction.len(), self.cur_block_size, self.cur_block_timeout);
        self.coa_lists.insert(round, coa_list);
        Ok(())
    }

    async fn handle_peer_blk_req(
        &mut self,
        _peer: NodeId,
        _msg: Vec<u8>,
    ) -> Result<(), CopycatError> {
        todo!()
    }

    fn report(&mut self) {}
}
