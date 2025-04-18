use super::{BlockManagement, CurBlockState};
use crate::config::DiemBasicConfig;
use crate::context::{BlkCtx, TxnCtx};
use crate::peers::PeerMessenger;
use crate::protocol::block::{Block, BlockHeader};
use crate::protocol::crypto::signature::P2PSignature;
use crate::protocol::crypto::threshold_signature::ThresholdSignature;
use crate::protocol::crypto::vector_snark::DummyMerkleTree;
use crate::protocol::crypto::{Hash, PrivKey, PubKey};
use crate::protocol::types::diem::{
    DiemBlock, QuorumCert, TimeCert, GENESIS_QC, GENESIS_VOTE_INFO,
};
use crate::protocol::MsgType;
use crate::stage::pacemaker::diem::DiemPacemaker;
use crate::stage::DelayPool;
use crate::transaction::{DiemAccountAddress, DiemTxn, Txn};
use crate::{CopycatError, NodeId, SignatureScheme};
use mailbox_client::SizedMsg;

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use tokio::sync::Notify;
use tokio::time::{Duration, Instant};

use async_trait::async_trait;
use dashmap::DashMap;
use lazy_static::lazy_static;
use primitive_types::U256;

lazy_static! {
    static ref BLK_SEND_TIMES: DashMap<Hash, Instant> = DashMap::new();
}

pub struct DiemBlockManagement {
    id: NodeId,
    blk_size: usize,
    _proposao_timeout: Duration,
    delay: Arc<DelayPool>,
    // states
    txn_pool: HashMap<Hash, (Arc<Txn>, Arc<TxnCtx>)>,
    accounts: HashMap<DiemAccountAddress, (PubKey, u64)>,
    state_merkle: HashMap<Hash, DummyMerkleTree>,
    // mempool
    pending_txns: VecDeque<Hash>,
    pending_txns_pool: HashSet<Hash>,
    // consensus info
    current_round: u64,
    high_qc: QuorumCert,
    high_commit_qc: QuorumCert,
    last_tc: Option<TimeCert>,
    qc_recv_time: Option<Instant>,
    // p2p communication
    all_nodes: Vec<NodeId>,
    peer_messenger: Arc<PeerMessenger>,
    signature_scheme: SignatureScheme,
    peer_pks: HashMap<NodeId, PubKey>,
    sk: PrivKey,
    threshold_signature: Arc<dyn ThresholdSignature>,
    // blocks pending validation
    pending_blks: HashMap<Hash, (Hash, u64)>, // parent_blk_id -> (blk_id, round)
    blk_pool: HashMap<Hash, (Arc<Block>, Arc<BlkCtx>)>, // only to respond to peer requests
    //
    _notify: Notify,
    // statistics
    blk_build_times: Vec<f64>,
    blk_deliver_times: Vec<f64>,
    blk_validation_times: Vec<f64>,
}

impl DiemBlockManagement {
    pub fn new(
        id: NodeId,
        p2p_signature: P2PSignature,
        threshold_signature: Arc<dyn ThresholdSignature>,
        config: DiemBasicConfig,
        delay: Arc<DelayPool>,
        peer_messenger: Arc<PeerMessenger>,
    ) -> Self {
        let (signature_scheme, peer_pks, sk) = p2p_signature;

        let mut all_nodes: Vec<NodeId> = peer_pks.keys().cloned().collect();
        all_nodes.sort();

        let genesis_qc = GENESIS_QC.clone();
        let current_round = 2;

        let qc_recv_time = if DiemPacemaker::get_leader(current_round, &all_nodes) == id {
            Some(Instant::now())
        } else {
            None
        };

        Self {
            id,
            blk_size: config.blk_size,
            _proposao_timeout: Duration::from_secs_f64(config.proposal_timeout_secs),
            delay,
            peer_messenger,
            txn_pool: HashMap::new(),
            blk_pool: HashMap::new(),
            accounts: HashMap::new(),
            state_merkle: HashMap::new(),
            pending_txns: VecDeque::new(),
            pending_txns_pool: HashSet::new(),
            current_round, // reserve round 0 and 1 for genesis
            high_qc: genesis_qc.clone(),
            high_commit_qc: genesis_qc,
            last_tc: None,
            qc_recv_time,
            all_nodes,
            signature_scheme,
            peer_pks,
            sk,
            threshold_signature,
            pending_blks: HashMap::new(),
            _notify: Notify::new(),
            blk_build_times: vec![],
            blk_deliver_times: vec![],
            blk_validation_times: vec![],
        }
    }

    async fn process_certificate_qc(&mut self, qc: &QuorumCert) -> Result<(), CopycatError> {
        let qc_round = qc.vote_info.round;

        if let Some(_id) = qc.commit_info.commit_state_id {
            // TODO: commit speculative execution results associated with id
            if qc_round > self.high_commit_qc.vote_info.round {
                self.high_commit_qc = qc.clone();
            }
        }

        if qc_round > self.high_qc.vote_info.round {
            self.high_qc = qc.clone();
            let chain_tail = qc.vote_info.blk_id;
            if chain_tail != GENESIS_VOTE_INFO.blk_id
                && !self.state_merkle.contains_key(&chain_tail)
            {
                self.request_blk(&chain_tail, qc_round).await?;
            }
        }

        if qc_round >= self.current_round {
            self.last_tc = None;
            self.current_round = qc_round + 1;
        }

        Ok(())
    }

    async fn process_certificate_tc(&mut self, tc: &Option<TimeCert>) -> Result<(), CopycatError> {
        if let Some(tc) = tc {
            let tc_round = tc.round;
            if tc_round >= self.current_round {
                self.last_tc = Some(tc.clone());
                self.current_round = tc_round + 1;
            }
        }

        Ok(())
    }

    fn validate_qc_signatures(&self, qc: &QuorumCert) -> Result<(bool, f64), CopycatError> {
        let mut verify_time = 0f64;

        let author_pk = match self.peer_pks.get(&qc.author) {
            Some(pk) => pk,
            None => return Ok((false, verify_time)),
        };

        let (check_result, dur) =
            self.signature_scheme
                .verify(author_pk, &qc.signatures, &qc.author_signature)?;
        verify_time += dur;
        if check_result == false {
            return Ok((false, verify_time));
        }

        let serialized_commit_info = bincode::serialize(&qc.commit_info)?;
        let (check_result, dur) = self
            .threshold_signature
            .verify(&serialized_commit_info, &qc.signatures)?;
        verify_time += dur;
        if check_result == false {
            return Ok((false, verify_time));
        }

        Ok((true, verify_time))
    }

    fn validate_tc_signatures(&self, tc: &Option<TimeCert>) -> Result<(bool, f64), CopycatError> {
        let tc = match tc {
            Some(tc) => tc,
            None => return Ok((true, 0f64)),
        };

        let mut verify_time = 0f64;

        for (node_id, high_qc_round) in &tc.tmo_high_qc_rounds {
            let author_pk = match self.peer_pks.get(node_id) {
                Some(pk) => pk,
                None => return Ok((false, verify_time)),
            };

            let data = (tc.round, *high_qc_round);
            let serialized = bincode::serialize(&data)?;
            let signature = match tc.tmo_signatures.get(node_id) {
                Some(sig) => sig,
                None => return Ok((false, verify_time)),
            };

            let (check_result, dur) =
                self.signature_scheme
                    .verify(author_pk, &serialized, &signature)?;
            verify_time += dur;
            if check_result == false {
                return Ok((false, verify_time));
            }
        }

        Ok((true, verify_time))
    }

    async fn request_blk(&self, blk_id: &Hash, round: u64) -> Result<(), CopycatError> {
        let leader = DiemPacemaker::get_leader(round, &self.all_nodes);
        pf_debug!(self.id; "requesting block {} from leader {}", blk_id, leader);
        let mut msg = vec![0u8; 32];
        blk_id.0.to_little_endian(&mut msg);
        self.peer_messenger
            .delayed_send(
                leader,
                MsgType::BlockReq { msg },
                Duration::from_secs_f64(self.delay.get_current_delay()),
            )
            .await
    }
}

#[async_trait]
impl BlockManagement for DiemBlockManagement {
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
        self.pending_txns.push_back(txn_id);
        self.pending_txns_pool.insert(txn_id);
        Ok(true)
    }

    // build the block only when we can propose a new block
    async fn prepare_new_block(&mut self) -> Result<CurBlockState, CopycatError> {
        Ok(CurBlockState::Full)
    }

    // when we receive a new QC from pmaker
    async fn wait_to_propose(&self) -> Result<(), CopycatError> {
        loop {
            // TODO: add timeout for batching
            if self.qc_recv_time.is_some() {
                let chain_tail = self.high_qc.vote_info.blk_id;
                if chain_tail == GENESIS_VOTE_INFO.blk_id
                    || self.state_merkle.contains_key(&chain_tail)
                {
                    return Ok(());
                }
            }
            self._notify.notified().await;
        }
    }

    // process_new_round_event()
    async fn get_new_block(&mut self) -> Result<(Arc<Block>, Arc<BlkCtx>), CopycatError> {
        let mut txns = vec![];
        let mut txn_ctxs = vec![];
        let mut blk_size = 0usize;
        let mut speculative_exec_time = 0f64;
        let mut speculative_exec_distinct_writes = 1usize; // 1 for proposer receiving gas
        let mut speculative_exec_distinct_inserts = 0usize;

        // speculative execute
        let mut merkle_tree = if self.high_qc.vote_info.blk_id == GENESIS_VOTE_INFO.blk_id {
            DummyMerkleTree::new()
        } else {
            self.state_merkle
                .get(&self.high_qc.vote_info.blk_id)
                .expect("missing depending blocks")
                .clone()
        };

        loop {
            let next_txn_id = match self.pending_txns.pop_front() {
                Some(txn) => txn,
                None => break, // no pending txns
            };

            if self.pending_txns_pool.remove(&next_txn_id) == false {
                continue; // txn already processed
            }

            let (next_txn, next_txn_ctx) = self.txn_pool.get(&next_txn_id).unwrap();

            let diem_txn = match next_txn.as_ref() {
                Txn::Diem { txn } => txn,
                _ => unreachable!(),
            };

            match diem_txn {
                DiemTxn::Txn {
                    sender,
                    seqno: _seqno, // TODO
                    payload,
                    max_gas_amount,
                    ..
                } => {
                    let (_, sender_balance) = match self.accounts.get(sender) {
                        Some(account) => account,
                        None => unreachable!(), // unknown sender - should not happen
                    };

                    if sender_balance < max_gas_amount {
                        continue; // invalid txn - not enough balance
                    }

                    speculative_exec_time += payload.script_runtime_sec;
                    if !payload.script_succeed {
                        continue; // invalid txn
                    }
                    // +1 for sender paying gas from balance
                    speculative_exec_distinct_writes += payload.distinct_writes + 1;
                }
                DiemTxn::Grant {
                    receiver,
                    receiver_key,
                    amount,
                } => match self.accounts.entry(*receiver) {
                    Entry::Occupied(mut e) => {
                        let (pubkey, balance) = e.get_mut();
                        if pubkey != receiver_key {
                            continue; // receivers do not match, ignore
                        }
                        *balance += amount; // TODO: move to speculative exec, do not change actual state yet
                    }
                    Entry::Vacant(e) => {
                        e.insert((receiver_key.clone(), *amount)); // TODO: move to speculative exec, do not change actual state yet
                        speculative_exec_distinct_inserts += 1; // TODO
                    }
                },
            }

            let txn_size = next_txn.size()?;
            txns.push(next_txn.clone());
            txn_ctxs.push(next_txn_ctx.clone());
            blk_size += txn_size;

            // block is full
            if blk_size >= self.blk_size {
                break;
            }
        }

        let append_dur = merkle_tree.append(speculative_exec_distinct_inserts)?;
        let update_dur = merkle_tree.update(speculative_exec_distinct_writes)?;
        self.delay
            .process_illusion(append_dur + update_dur + speculative_exec_time)
            .await;

        // build block
        let diem_blk = DiemBlock {
            proposer: self.id,
            round: self.current_round,
            state_id: merkle_tree.get_root(),
            qc: self.high_qc.clone(),
        };
        let diem_blk_id = diem_blk.compute_id(&txn_ctxs)?;
        self.state_merkle.insert(diem_blk_id, merkle_tree);

        let serialized = bincode::serialize(&diem_blk_id)?;
        let (signature, dur) = self.signature_scheme.sign(&self.sk, &serialized)?;
        let last_tc = std::mem::replace(&mut self.last_tc, None);
        self.delay.process_illusion(dur).await;
        let blk_header = BlockHeader::Diem {
            block: diem_blk,
            high_commit_qc: self.high_commit_qc.clone(),
            last_round_tc: last_tc,
            signature,
        };
        let blk = Arc::new(Block {
            header: blk_header,
            txns,
        });
        let blk_ctx = Arc::new(BlkCtx::from_id_and_txns(diem_blk_id, txn_ctxs));

        self.blk_pool
            .insert(diem_blk_id, (blk.clone(), blk_ctx.clone()));

        let blk_build_instant = Instant::now();
        BLK_SEND_TIMES.insert(diem_blk_id, blk_build_instant);
        let blk_build_time = blk_build_instant - self.qc_recv_time.unwrap();
        self.blk_build_times.push(blk_build_time.as_secs_f64());
        self.qc_recv_time = None;

        Ok((blk, blk_ctx))
    }

    async fn validate_block(
        &mut self,
        src: NodeId,
        block: Arc<Block>,
        ctx: Arc<BlkCtx>,
    ) -> Result<Vec<(Arc<Block>, Arc<BlkCtx>)>, CopycatError> {
        // record block delivery time
        let blk_recv_time = Instant::now();
        let blk_send_time = *BLK_SEND_TIMES.get(&ctx.id).unwrap();
        self.blk_deliver_times
            .push((blk_recv_time - blk_send_time).as_secs_f64());

        let (diem_blk, high_commit_qc, last_round_tc, signature) = match &block.header {
            BlockHeader::Diem {
                block,
                high_commit_qc,
                last_round_tc,
                signature,
            } => (block, high_commit_qc, last_round_tc, signature),
            _ => unreachable!(),
        };

        // validate signatures
        let qc_valid = if diem_blk.qc == *GENESIS_QC {
            true
        } else {
            let (valid, verify_time) = self.validate_qc_signatures(&diem_blk.qc)?;
            self.delay.process_illusion(verify_time).await;
            if valid {
                self.process_certificate_qc(&diem_blk.qc).await?;
            }
            valid
        };

        let high_commit_qc_valid = if *high_commit_qc == *GENESIS_QC {
            true
        } else {
            let (valid, verify_time) = self.validate_qc_signatures(high_commit_qc)?;
            self.delay.process_illusion(verify_time).await;
            if valid {
                self.process_certificate_qc(high_commit_qc).await?;
            }
            valid
        };

        let tc_valid = {
            let (valid, verify_time) = self.validate_tc_signatures(last_round_tc)?;
            self.delay.process_illusion(verify_time).await;
            if valid {
                self.process_certificate_tc(last_round_tc).await?;
            }
            valid
        };

        let signature_valid = match self.peer_pks.get(&diem_blk.proposer) {
            Some(pk) => {
                let serialized = bincode::serialize(&ctx.id)?;
                let (valid, verify_time) =
                    self.signature_scheme.verify(&pk, &serialized, signature)?;
                self.delay.process_illusion(verify_time).await;
                valid
            }
            None => false,
        };

        if !(qc_valid && high_commit_qc_valid && tc_valid && signature_valid) {
            return Ok(vec![]);
        }

        let leader = DiemPacemaker::get_leader(diem_blk.round, &self.all_nodes);
        if diem_blk.proposer != leader || src != leader {
            return Ok(vec![]);
        }

        self.blk_pool.insert(ctx.id, (block.clone(), ctx.clone()));

        // speculative exec
        let mut speculative_exec_time = 0f64;
        let mut speculative_exec_distinct_writes = 1usize;
        let mut speculative_exec_distinct_inserts = 0usize;

        let parent_id = &diem_blk.qc.vote_info.blk_id;
        let parent_round = diem_blk.qc.vote_info.round;
        let mut merkle_tree = if *parent_id == GENESIS_VOTE_INFO.blk_id {
            DummyMerkleTree::new()
        } else {
            match self.state_merkle.get(parent_id) {
                Some(merkle_tree) => merkle_tree.clone(),
                None => {
                    self.request_blk(parent_id, parent_round).await?; // missing parent block, will continue validate when we get all dependencies
                    match self.pending_blks.entry(*parent_id) {
                        Entry::Occupied(mut e) => {
                            let (blk_id, round) = e.get_mut();
                            // only keep track of latest block
                            if *round < diem_blk.round {
                                *round = diem_blk.round;
                                *blk_id = ctx.id;
                            }
                        }
                        Entry::Vacant(e) => {
                            e.insert((ctx.id, diem_blk.round));
                        }
                    }
                    return Ok(vec![]);
                }
            }
        };

        let mut new_tail = vec![];

        let mut cur_block = block;
        let mut cur_block_ctx = ctx;

        loop {
            let diem_blk = match &cur_block.header {
                BlockHeader::Diem { block, .. } => block,
                _ => unreachable!(),
            };
            let cur_blk_id = cur_block_ctx.id;
            cur_block_ctx = if diem_blk.round == self.current_round {
                cur_block_ctx
            } else {
                Arc::new(cur_block_ctx.invalidate())
            };

            for txn in &cur_block.txns {
                let diem_txn = match txn.as_ref() {
                    Txn::Diem { txn } => txn,
                    _ => unreachable!(),
                };

                match diem_txn {
                    DiemTxn::Txn {
                        sender,
                        seqno: _seqno, // TODO
                        payload,
                        max_gas_amount,
                        ..
                    } => {
                        let (_, sender_balance) = match self.accounts.get(sender) {
                            Some(account) => account,
                            None => return Ok(vec![]), // invalid txn - unknown sender
                        };

                        if sender_balance < max_gas_amount {
                            return Ok(vec![]); // invalid txn - not enough balance
                        }

                        speculative_exec_time += payload.script_runtime_sec;
                        if !payload.script_succeed {
                            return Ok(vec![]); // invalid txn
                        }
                        // +1 for sender paying gas from balance
                        speculative_exec_distinct_writes += payload.distinct_writes + 1;
                    }
                    DiemTxn::Grant {
                        receiver,
                        receiver_key,
                        amount,
                    } => match self.accounts.entry(*receiver) {
                        Entry::Occupied(mut e) => {
                            let (pubkey, balance) = e.get_mut();
                            if pubkey != receiver_key {
                                continue; // receivers do not match, ignore
                            }
                            *balance += amount; // TODO: move to speculative exec, do not change actual state yet
                        }
                        Entry::Vacant(e) => {
                            e.insert((receiver_key.clone(), *amount)); // TODO: move to speculative exec, do not change actual state yet
                            speculative_exec_distinct_inserts += 1; // TODO
                        }
                    },
                }
            }

            let append_dur = merkle_tree.append(speculative_exec_distinct_inserts)?;
            let update_dur = merkle_tree.update(speculative_exec_distinct_writes)?;
            self.delay
                .process_illusion(append_dur + update_dur + speculative_exec_time)
                .await;
            if !merkle_tree.verify_root(&diem_blk.state_id)? {
                return Ok(vec![]); // speculative exec results do not match
            }
            pf_debug!(self.id; "validated block {}", cur_blk_id);
            self.state_merkle.insert(cur_blk_id, merkle_tree.clone()); // TODO: merkle tree copy may be expensive

            // remove txns from mempool
            for txn_ctx in &cur_block_ctx.txn_ctx {
                self.pending_txns_pool.remove(&txn_ctx.id);
            }
            new_tail.push((cur_block, cur_block_ctx));
            // check pending blocks if found
            (cur_block, cur_block_ctx) = match self.pending_blks.remove(&cur_blk_id) {
                Some((blk_id, _)) => self.blk_pool.get(&blk_id).unwrap().clone(),
                None => break,
            }
        }

        let blk_validation_time = Instant::now();
        self.blk_validation_times
            .push((blk_validation_time - blk_recv_time).as_secs_f64());
        Ok(new_tail)
    }

    async fn handle_pmaker_msg(&mut self, msg: Vec<u8>) -> Result<(), CopycatError> {
        let qc: QuorumCert = bincode::deserialize(&msg)?;
        self.process_certificate_qc(&qc).await?;
        self.qc_recv_time = Some(Instant::now());
        Ok(())
    }

    async fn handle_peer_blk_req(
        &mut self,
        peer: NodeId,
        msg: Vec<u8>,
    ) -> Result<(), CopycatError> {
        let blk_id = Hash(U256::from_little_endian(&msg));
        let blk = match self.blk_pool.get(&blk_id) {
            Some((blk, _)) => blk.clone(),
            None => return Ok(()), // block not found
        };

        pf_debug!(self.id; "sending block {} to {}", blk_id, peer);

        self.peer_messenger
            .delayed_send(
                peer,
                MsgType::NewBlock { blk },
                Duration::from_secs_f64(self.delay.get_current_delay()),
            )
            .await
    }

    fn report(&mut self) {
        let blk_building_times = std::mem::replace(&mut self.blk_build_times, vec![]);
        let blk_delivery_times = std::mem::replace(&mut self.blk_deliver_times, vec![]);
        let blk_validating_times = std::mem::replace(&mut self.blk_validation_times, vec![]);
        let avg_blk_building_time =
            blk_building_times.iter().sum::<f64>() / blk_building_times.len() as f64;
        let avg_blk_validation_time =
            blk_validating_times.iter().sum::<f64>() / blk_validating_times.len() as f64;
        pf_info!(self.id; "avg_blk_building_time: {}, avg_blk_validation_time: {}", avg_blk_building_time, avg_blk_validation_time);
        pf_info!(self.id; "blk_delivery_times: {:?}", blk_delivery_times);
    }
}

#[cfg(test)]
mod diem_block_management_test {
    use std::collections::{HashMap, HashSet};
    use std::sync::Arc;

    use primitive_types::U256;

    use super::DiemBlockManagement;
    use crate::context::BlkCtx;
    use crate::peers::PeerMessenger;
    use crate::protocol::block::{Block, BlockHeader};
    use crate::protocol::crypto::{sha256, Hash};
    use crate::protocol::types::diem::{
        DiemBlock, LedgerCommitInfo, QuorumCert, VoteInfo, GENESIS_QC,
    };
    use crate::stage::consensus::block_management::BlockManagement;
    use crate::stage::pacemaker::diem::DiemPacemaker;
    use crate::stage::DelayPool;
    use crate::{CopycatError, NodeId, SignatureScheme, ThresholdSignatureScheme};

    #[tokio::test]
    async fn test_blocks_arrive_out_of_order() -> Result<(), CopycatError> {
        let p2p_signature_scheme = SignatureScheme::Dummy;
        let threshold_signature_scheme = ThresholdSignatureScheme::Dummy;

        let node_list = vec![0, 1, 2, 3];

        let all_nodes = HashSet::from_iter(node_list.iter().cloned());
        let p2p_signature = p2p_signature_scheme.gen_p2p_signature(0, all_nodes.iter());
        let threshold_signatures =
            threshold_signature_scheme.to_threshold_signature(&all_nodes, 3, 0)?;
        let threshold_signature = threshold_signatures.get(&0).unwrap().clone();

        let config = Default::default();
        let delay = Arc::new(DelayPool::new());
        let peer_messenger = PeerMessenger::new_stub();

        let mut diem = DiemBlockManagement::new(
            0,
            p2p_signature,
            threshold_signature,
            config,
            delay,
            Arc::new(peer_messenger),
        );

        let blk1_leader = DiemPacemaker::get_leader(2, &node_list);
        let diem_blk1 = DiemBlock {
            proposer: blk1_leader,
            round: 2,
            state_id: Hash(U256::zero()),
            qc: GENESIS_QC.clone(),
        };
        let blk1_header = BlockHeader::Diem {
            block: diem_blk1,
            last_round_tc: None,
            high_commit_qc: GENESIS_QC.clone(),
            signature: vec![],
        };
        let blk1 = Arc::new(Block {
            header: blk1_header,
            txns: vec![],
        });
        let blk1_ctx = Arc::new(BlkCtx::from_blk(&blk1)?);

        let blk2_leader = DiemPacemaker::get_leader(3, &node_list);
        let blk1_qc_vinfo = VoteInfo {
            blk_id: blk1_ctx.id,
            round: 2,
            parent_id: GENESIS_QC.vote_info.blk_id,
            parent_round: GENESIS_QC.vote_info.round,
            exec_state_hash: Hash(U256::zero()),
        };
        let blk1_qc_vinfo_hash = sha256(&blk1_qc_vinfo)?;
        let mut blk1_qc_signatures = (1..4 as NodeId)
            .map(|node| (node, vec![]))
            .collect::<HashMap<_, _>>();
        let blk2_leader_thresh = threshold_signatures.get(&blk2_leader).unwrap();
        let (blk1_qc_signatures_agg, _) =
            blk2_leader_thresh.aggregate(&[0], &mut blk1_qc_signatures)?;
        let blk1_qc = QuorumCert {
            vote_info: blk1_qc_vinfo,
            commit_info: LedgerCommitInfo {
                commit_state_id: None,
                vote_info_hash: blk1_qc_vinfo_hash,
            },
            signatures: blk1_qc_signatures_agg.unwrap(),
            author: blk2_leader,
            author_signature: vec![],
        };
        let diem_blk2 = DiemBlock {
            proposer: blk2_leader,
            round: 3,
            state_id: Hash(U256::zero()),
            qc: blk1_qc,
        };
        let blk2_header = BlockHeader::Diem {
            block: diem_blk2,
            last_round_tc: None,
            high_commit_qc: GENESIS_QC.clone(),
            signature: vec![],
        };
        let blk2 = Arc::new(Block {
            header: blk2_header,
            txns: vec![],
        });
        let blk2_ctx = Arc::new(BlkCtx::from_blk(&blk2)?);

        assert!(diem
            .validate_block(blk2_leader, blk2, blk2_ctx.clone())
            .await?
            .is_empty());
        let ret = diem
            .validate_block(blk1_leader, blk1, blk1_ctx.clone())
            .await?;
        assert!(ret.len() == 2);
        assert!(ret[0].1.id == blk1_ctx.id);
        assert!(ret[1].1.id == blk2_ctx.id);

        Ok(())
    }
}
