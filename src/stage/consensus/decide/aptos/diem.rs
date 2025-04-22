use crate::config::AptosDiemConfig;
use crate::context::{BlkCtx, BlkData, TxnCtx};
use crate::peers::PeerMessenger;
use crate::protocol::block::{Block, BlockHeader};
use crate::protocol::crypto::signature::P2PSignature;
use crate::protocol::crypto::threshold_signature::{SignPart, ThresholdSignature};
use crate::protocol::crypto::vector_snark::DummyMerkleTree;
use crate::protocol::crypto::{sha256, Hash, PrivKey, PubKey, Signature};
use crate::protocol::types::aptos::CoA;
use crate::protocol::types::diem::{
    DiemBlock, LedgerCommitInfo, QuorumCert, TimeCert, VoteInfo, GENESIS_QC, GENESIS_VOTE_INFO,
};
use crate::protocol::MsgType;
use crate::stage::consensus::decide::Decision;
use crate::stage::DelayPool;
use crate::transaction::Txn;
use crate::{CopycatError, NodeId, SignatureScheme};

use async_trait::async_trait;
use dashmap::DashMap;
use lazy_static::lazy_static;
use mailbox_client::SizedMsg;
use primitive_types::U256;
use serde::{Deserialize, Serialize};

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use tokio::sync::Notify;
use tokio::time::{Duration, Instant};

lazy_static! {
    static ref PROPOSAL_SEND_TIME: DashMap<Hash, Instant> = DashMap::new();
    static ref VOTE_SEND_TIME: DashMap<(Hash, NodeId), Instant> = DashMap::new();
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum DiemConsensusMsg {
    Proposal {
        block: DiemBlock,
        last_round_tc: Option<TimeCert>,
        high_commit_qc: QuorumCert,
        signature: Signature,
        payload: Vec<CoA>,
    },
    Vote {
        vote_info: VoteInfo,
        ledger_commit_info: LedgerCommitInfo,
        high_commit_qc: QuorumCert,
        sender: NodeId,
        signature: SignPart,
    },
    FetchBlockReq {
        blk_id: Hash,
    },
    FetchBlockResp {
        blk: DiemBlock,
        payload: Vec<CoA>,
    },
    FetchBatchReq {
        sender: NodeId,
        round: u64,
    },
    FetchBatchResp {
        batch: Arc<Block>,
    },
}

#[derive(Hash, PartialEq, Eq)]
enum PendingVoteDep {
    Round { round: u64 },
    Parent { blk_id: Hash },
}

pub struct AptosDiemDecision {
    id: NodeId,
    num_faulty: usize,
    blk_len: usize,
    blk_size: usize,
    proposal_timeout: Duration,
    _vote_timeout: Duration, // TODO
    //
    quorum_store: HashMap<(NodeId, u64), (Arc<Block>, Arc<BlkCtx>)>,
    pending_queue: VecDeque<(NodeId, u64)>,
    committed_pool: HashSet<(NodeId, u64)>,
    block_under_construction: Vec<CoA>,
    cur_block_size: usize,
    cur_proposal_timeout: Option<Instant>,
    // diem
    blk_pool: HashMap<Hash, (DiemBlock, Vec<CoA>)>,
    current_round: u64,
    high_qc: QuorumCert,
    high_commit_qc: QuorumCert,
    last_tc: Option<TimeCert>,
    highest_qc_round: u64,
    highest_vote_round: u64,
    received_votes: HashMap<Hash, HashMap<NodeId, SignPart>>,
    pending_votes: HashMap<PendingVoteDep, (Hash, u64, Hash, u64, Hash)>,
    // P2P communication
    all_nodes: Vec<NodeId>,
    peer_messenger: Arc<PeerMessenger>,
    signature_scheme: SignatureScheme,
    peer_pks: HashMap<NodeId, PubKey>,
    sk: PrivKey,
    threshold_signature: Arc<dyn ThresholdSignature>,
    //
    commit_ready_blks: VecDeque<Hash>,
    commit_waiting_blks: HashMap<Hash, Hash>,
    committed_blk_pool: HashSet<Hash>,
    commit_queue: VecDeque<(NodeId, u64)>,
    commit_depth: u64,
    queried_blks: HashSet<Hash>,
    queried_batchs: HashMap<(NodeId, u64), CoA>,
    delay: Arc<DelayPool>,
    _notify: Notify,
    // statistics
    qc_form_time: Option<Instant>,
    blk_build_times: Vec<f64>,
    blk_send_times: Vec<f64>,
    blk_validate_times: Vec<f64>,
    vote_send_times: Vec<f64>,
    vote_validate_times: Vec<f64>,
    round_latency: Vec<f64>,
}

impl AptosDiemDecision {
    fn get_leader(round: u64, all_nodes: &Vec<NodeId>) -> NodeId {
        let idx = (round / 2) % all_nodes.len() as u64;
        all_nodes[idx as usize]
    }

    pub fn new(
        id: NodeId,
        p2p_signature: P2PSignature,
        threshold_signature: Arc<dyn ThresholdSignature>,
        config: AptosDiemConfig,
        peer_messenger: Arc<PeerMessenger>,
        delay: Arc<DelayPool>,
    ) -> Self {
        let (signature_scheme, peer_pks, sk) = p2p_signature;

        let mut all_nodes: Vec<NodeId> = peer_pks.keys().cloned().collect();
        all_nodes.sort();

        let proposal_timeout = Duration::from_secs_f64(config.diem_batching_timeout_secs);
        let vote_timeout = Duration::from_secs_f64(config.diem_vote_timeout_secs);

        let current_round = 2;
        let (qc_form_time, cur_proposal_timeout) =
            if Self::get_leader(current_round, &all_nodes) == id {
                let current = Instant::now();
                (Some(current), Some(current + proposal_timeout))
            } else {
                (None, None)
            };

        Self {
            id,
            num_faulty: (all_nodes.len() + 1) / 3,
            blk_len: config.diem_blk_len,
            blk_size: config.diem_blk_size,
            proposal_timeout,
            _vote_timeout: vote_timeout,
            quorum_store: HashMap::new(),
            pending_queue: VecDeque::new(),
            committed_pool: HashSet::new(),
            block_under_construction: vec![],
            cur_block_size: 0,
            cur_proposal_timeout,
            blk_pool: HashMap::new(),
            current_round,
            high_qc: GENESIS_QC.clone(),
            high_commit_qc: GENESIS_QC.clone(),
            last_tc: None,
            highest_qc_round: 0,
            highest_vote_round: 0,
            received_votes: HashMap::new(),
            pending_votes: HashMap::new(),
            all_nodes,
            peer_messenger,
            signature_scheme,
            peer_pks: peer_pks,
            sk,
            threshold_signature,
            commit_ready_blks: VecDeque::new(),
            commit_waiting_blks: HashMap::new(),
            committed_blk_pool: HashSet::from([
                GENESIS_VOTE_INFO.blk_id,
                GENESIS_VOTE_INFO.parent_id,
            ]),
            commit_queue: VecDeque::new(),
            commit_depth: 0,
            queried_blks: HashSet::new(),
            queried_batchs: HashMap::new(),
            delay,
            _notify: Notify::new(),
            qc_form_time,
            blk_build_times: vec![],
            blk_send_times: vec![],
            blk_validate_times: vec![],
            vote_send_times: vec![],
            vote_validate_times: vec![],
            round_latency: vec![],
        }
    }

    #[inline]
    fn is_cur_block_full(&self) -> bool {
        self.block_under_construction.len() >= self.blk_len || self.cur_block_size >= self.blk_size
    }

    #[inline]
    fn has_batched_enough(&self) -> bool {
        self.pending_queue.len() + self.block_under_construction.len() >= self.blk_len
            || self.pending_queue.len()
            * ((self.all_nodes.len() - self.num_faulty) * 64 + 48)  // size of each CoA
            + self.cur_block_size
                >= self.blk_size
    }

    #[inline]
    fn blk_seen(&self, blk_id: &Hash) -> bool {
        *blk_id == GENESIS_VOTE_INFO.blk_id
            || *blk_id == GENESIS_VOTE_INFO.parent_id
            || self.blk_pool.contains_key(blk_id)
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

    async fn process_certificate_qc(&mut self, qc: &QuorumCert) -> Result<(), CopycatError> {
        let qc_round = qc.vote_info.round;
        if let Some(grandparent_id) = qc.commit_info.commit_state_id {
            // TODO: commit all parent blocks
            self.add_to_commit_ready(qc.vote_info.parent_id, grandparent_id)?;
            if qc_round > self.high_commit_qc.vote_info.round {
                self.high_commit_qc = qc.clone();
            }
        }

        if qc_round > self.high_qc.vote_info.round {
            self.high_qc = qc.clone();
        }

        // advance round
        if qc_round >= self.current_round {
            // TODO: set timeouts
            self.last_tc = None;
            self.current_round = qc_round + 1;
        }

        Ok(())
    }

    fn process_certificate_tc(&mut self, tc: &Option<TimeCert>) -> Result<(), CopycatError> {
        if let Some(tc) = tc {
            let tc_round = tc.round;
            if tc_round >= self.current_round {
                self.last_tc = Some(tc.clone());
                self.current_round = tc_round + 1;
            }
        }

        Ok(())
    }

    async fn make_vote(
        &mut self,
        blk_id: Hash,
        round: u64,
        parent_id: Hash,
        parent_round: u64,
        grand_parent_id: Hash,
    ) -> Result<(), CopycatError> {
        pf_debug!(self.id; "voting for new block: {}", blk_id);

        if self.highest_qc_round < parent_round {
            self.highest_qc_round = parent_round;
        }

        if self.highest_vote_round < round {
            self.highest_vote_round = round;
        }

        let commit_state_id = if parent_round + 1 == round {
            Some(grand_parent_id)
        } else {
            None
        };

        let next_leader = Self::get_leader(round + 1, &self.all_nodes);
        let vote_info = VoteInfo {
            blk_id,
            round,
            parent_id,
            parent_round,
            exec_state_hash: Hash(U256::zero()),
        };

        let ledger_commit_info = LedgerCommitInfo {
            commit_state_id,
            vote_info_hash: sha256(&vote_info)?,
        };

        let vote_id = sha256(&ledger_commit_info)?;
        let (signature, dur) = self
            .threshold_signature
            .sign(&bincode::serialize(&vote_id)?)?;
        self.delay.process_illusion(dur).await;

        if next_leader == self.id {
            self.record_vote(vote_info, ledger_commit_info, self.id, signature)
                .await?;
        } else {
            VOTE_SEND_TIME.insert((blk_id, self.id), Instant::now());
            let vote_msg = DiemConsensusMsg::Vote {
                vote_info,
                ledger_commit_info,
                high_commit_qc: self.high_commit_qc.clone(),
                sender: self.id,
                signature,
            };
            self.peer_messenger
                .send(
                    next_leader,
                    MsgType::ConsensusMsg {
                        msg: bincode::serialize(&vote_msg)?,
                    },
                )
                .await?;
        }
        Ok(())
    }

    async fn record_vote(
        &mut self,
        vote_info: VoteInfo,
        ledger_commit_info: LedgerCommitInfo,
        voter: NodeId,
        signature: SignPart,
    ) -> Result<(), CopycatError> {
        let vote_id = sha256(&ledger_commit_info)?;

        let votes = self.received_votes.entry(vote_id).or_insert(HashMap::new());
        votes.insert(voter, signature);

        let signcomb = match self
            .threshold_signature
            .aggregate(&bincode::serialize(&vote_id)?, votes)
        {
            Ok((signature, dur)) => {
                self.delay.process_illusion(dur).await;
                signature
            }
            Err(e) => {
                pf_warn!(self.id; "failed to aggregate votes: {:?}", e);
                self.received_votes.remove(&vote_id);
                // self.closed_votes.insert(vote_id);
                return Ok(());
            }
        };

        let qc = match signcomb {
            Some(signature) => {
                let (author_signature, dur) =
                    self.signature_scheme.sign(&self.sk, &signature).unwrap();
                self.delay.process_illusion(dur).await;
                self.received_votes.remove(&vote_id);
                QuorumCert {
                    vote_info,
                    commit_info: ledger_commit_info,
                    signatures: signature,
                    author: self.id,
                    author_signature,
                }
            }
            None => return Ok(()),
        };

        pf_debug!(self.id; "formed qc for block {}", qc.vote_info.blk_id);
        self.qc_form_time = Some(Instant::now());

        self.process_certificate_qc(&qc).await?;
        self.cur_proposal_timeout = Some(Instant::now() + self.proposal_timeout);
        if !self.blk_seen(&qc.vote_info.blk_id) {
            // TODO: fetch the missing block
        }

        Ok(())
    }

    fn add_to_commit_ready(&mut self, blk_id: Hash, parent_id: Hash) -> Result<(), CopycatError> {
        if self.committed_blk_pool.contains(&blk_id) {
            return Ok(());
        }

        let mut cur_blk_id = blk_id;
        let mut cur_parent_id = parent_id;

        loop {
            if self.committed_blk_pool.contains(&cur_parent_id) {
                // this parent is already committed, commit cur_blk_id and all its children
                let mut next_to_commit = cur_blk_id;
                loop {
                    pf_debug!(self.id; "adding block {} to commit queue", next_to_commit);
                    self.commit_ready_blks.push_back(next_to_commit);
                    self.committed_blk_pool.insert(next_to_commit);
                    next_to_commit = match self.commit_waiting_blks.remove(&next_to_commit) {
                        Some(blk) => blk,
                        None => return Ok(()),
                    }
                }
            }

            if let Some(sibling) = self.commit_waiting_blks.get(&cur_parent_id) {
                // otherwise we have a conflict
                assert!(*sibling == cur_blk_id);
                // all ancestors are already in pending, wait for some ancestor block before committing
                return Ok(());
            }

            self.commit_waiting_blks.insert(cur_parent_id, cur_blk_id);

            cur_blk_id = cur_parent_id;
            cur_parent_id = match self.blk_pool.get(&cur_blk_id) {
                Some((blk, _)) => blk.qc.vote_info.blk_id,
                None => return Ok(()), // we do not know who the parent is, wait to get block with cur_blk_id - TODO: actively fetch the block
            }
        }
    }

    async fn handle_commit_ready_blks(&mut self) -> Result<(), CopycatError> {
        loop {
            let next_blk_id = match self.commit_ready_blks.front() {
                Some(id) => id,
                None => return Ok(()), // nothing to commit
            };

            let (_, next_blk_payload) = match self.blk_pool.get(next_blk_id) {
                Some(blk) => blk,
                None => {
                    if !self.queried_blks.contains(&next_blk_id) {
                        // TODO: do not query blocks yet since it may arrive soon
                        pf_debug!(self.id; "fetch block {:?} from peers", next_blk_id);
                        let msg = DiemConsensusMsg::FetchBlockReq {
                            blk_id: *next_blk_id,
                        };
                        self.peer_messenger
                            .broadcast(MsgType::ConsensusMsg {
                                msg: bincode::serialize(&msg)?,
                            })
                            .await?;
                        self.queried_blks.insert(*next_blk_id);
                    }
                    return Ok(());
                }
            };

            self.commit_ready_blks.pop_front();

            // add payloads to pending queue
            for batch in next_blk_payload {
                let batch_key = (batch.sender, batch.round);
                self.commit_queue.push_back(batch_key);
                if !self.quorum_store.contains_key(&batch_key) {
                    if !self.queried_batchs.contains_key(&batch_key) {
                        // request block from batch sender
                        let msg = DiemConsensusMsg::FetchBatchReq {
                            sender: batch.sender,
                            round: batch.round,
                        };
                        self.peer_messenger
                            .send(
                                batch.sender,
                                MsgType::ConsensusMsg {
                                    msg: bincode::serialize(&msg)?,
                                },
                            )
                            .await?;
                        self.queried_batchs.insert(batch_key, batch.clone());
                    }
                }
            }
        }
    }

    async fn process_waiting_actions(
        &mut self,
        blk_id: Hash,
        parent_id: Hash,
    ) -> Result<(), CopycatError> {
        // process commit waiting blocks
        if self.commit_waiting_blks.contains_key(&blk_id) {
            self.add_to_commit_ready(blk_id, parent_id)?;
        }

        // process commit queue
        self.handle_commit_ready_blks().await?;

        // make pending votes
        if let Some((blk_id, round, parent_id, parent_round, grandparent_id)) = self
            .pending_votes
            .remove(&PendingVoteDep::Parent { blk_id })
        {
            if round > self.current_round {
                self.pending_votes.insert(
                    PendingVoteDep::Round { round: round },
                    (blk_id, round, parent_id, parent_round, grandparent_id),
                );
            } else {
                self.make_vote(blk_id, round, parent_id, parent_round, grandparent_id)
                    .await?;
            }
        }

        Ok(())
    }
}

#[async_trait]
impl Decision for AptosDiemDecision {
    async fn new_tail(
        &mut self,
        _src: NodeId,
        new_tail: Vec<(Arc<Block>, Arc<BlkCtx>)>,
    ) -> Result<(), CopycatError> {
        for (blk, ctx) in new_tail {
            let (sender, round) = match blk.header {
                BlockHeader::Aptos { sender, round, .. } => (sender, round),
                _ => unreachable!(),
            };
            match self.quorum_store.entry((sender, round)) {
                Entry::Occupied(mut e) => {
                    let (orig_blk, orig_ctx) = e.get_mut();
                    if orig_blk.txns.len() == 0 {
                        *orig_blk = blk;
                        *orig_ctx = ctx;
                    }
                }
                Entry::Vacant(e) => {
                    e.insert((blk, ctx));
                    if !self.committed_pool.contains(&(sender, round)) {
                        self.pending_queue.push_back((sender, round));
                    }
                }
            }
        }
        Ok(())
    }

    async fn commit_ready(&self) -> Result<(), CopycatError> {
        if let Some(batch) = self.commit_queue.front() {
            if self.quorum_store.contains_key(batch) {
                return Ok(());
            }
        }

        loop {
            self._notify.notified().await;
        }
    }

    async fn next_to_commit(
        &mut self,
    ) -> Result<(u64, (Vec<Arc<Txn>>, Vec<Arc<TxnCtx>>)), CopycatError> {
        let batch_key = self.commit_queue.pop_front().unwrap();
        let (batch_blk, batch_ctx) = self.quorum_store.get(&batch_key).unwrap();
        self.commit_depth += 1;
        return Ok((
            self.commit_depth,
            (batch_blk.txns.clone(), batch_ctx.txn_ctx.clone()),
        ));
    }

    async fn timeout(&self) -> Result<(), CopycatError> {
        // propose only if I am leader
        if let Some(timeout) = self.cur_proposal_timeout {
            // make sure we have seen the parent before proposing
            if self.blk_seen(&self.high_qc.vote_info.blk_id) {
                // already enough content, start proposing
                if self.has_batched_enough() {
                    return Ok(());
                }

                // wait for timeout
                if timeout > Instant::now() + Duration::from_millis(1) {
                    tokio::time::sleep_until(timeout).await;
                }

                return Ok(());
            }
        }

        // we have advanced round and need to make a vote
        if self.pending_votes.contains_key(&PendingVoteDep::Round {
            round: self.current_round,
        }) {
            return Ok(());
        }

        // TODO: add vote timeout

        loop {
            self._notify.notified().await;
        }
    }

    async fn handle_timeout(&mut self) -> Result<(), CopycatError> {
        let current = Instant::now();
        if let Some(timeout) = self.cur_proposal_timeout {
            if self.has_batched_enough() || timeout <= current + Duration::from_millis(1) {
                // propose new block
                loop {
                    // TODO: order blocks according to dependency (certificates)
                    let next_batch = match self.pending_queue.pop_front() {
                        Some(batch) => batch,
                        None => break,
                    };

                    if self.committed_pool.contains(&next_batch) {
                        // batch has already committed
                        continue;
                    }

                    self.committed_pool.insert(next_batch);

                    let (_, ctx) = self.quorum_store.get(&next_batch).unwrap();
                    let certificate = match ctx.data.as_ref().unwrap() {
                        BlkData::Aptos { certificate } => certificate,
                    };
                    self.cur_block_size += certificate.size()?;
                    self.block_under_construction.push(certificate.clone()); // TODO: remove clone()
                    if self.is_cur_block_full() {
                        break;
                    }
                }

                // do not propose yet if block is not full and timeout has not reached
                if !self.is_cur_block_full() && timeout > current + Duration::from_millis(1) {
                    return Ok(());
                }

                let blk_len = self.block_under_construction.len();
                let payload = std::mem::replace(
                    &mut self.block_under_construction,
                    Vec::with_capacity(blk_len),
                );
                let round = self.current_round;
                let parent_id = self.high_qc.vote_info.blk_id;
                let parent_round = self.high_qc.vote_info.round;
                let grand_parent_id = self.high_qc.vote_info.parent_id;

                let diem_blk = DiemBlock {
                    proposer: self.id,
                    round,
                    state_id: Hash(U256::zero()),
                    qc: self.high_qc.clone(),
                };
                let blk_id = diem_blk.compute_id(&payload)?;
                let serialized = bincode::serialize(&blk_id)?;
                let (signature, dur) = self.signature_scheme.sign(&self.sk, &serialized)?;
                self.delay.process_illusion(dur).await;
                let last_round_tc = std::mem::replace(&mut self.last_tc, None);

                pf_debug!(self.id; "round {} proposing new block {}: num_batches: {}, content: {:?}", round, blk_id, payload.len(), payload);

                let proposal = DiemConsensusMsg::Proposal {
                    block: diem_blk,
                    last_round_tc,
                    high_commit_qc: self.high_commit_qc.clone(),
                    signature,
                    payload,
                };

                let proposal_time = Instant::now();
                let block_building_time = self.qc_form_time.unwrap() - proposal_time;
                self.blk_build_times.push(block_building_time.as_secs_f64());
                if parent_id != GENESIS_VOTE_INFO.blk_id {
                    let parent_proposal_time = *PROPOSAL_SEND_TIME.get(&parent_id).unwrap();
                    let round_time = proposal_time - parent_proposal_time;
                    self.round_latency.push(round_time.as_secs_f64());
                }
                self.qc_form_time = None;
                PROPOSAL_SEND_TIME.insert(blk_id, proposal_time);

                self.peer_messenger
                    .broadcast(MsgType::ConsensusMsg {
                        msg: bincode::serialize(&proposal)?,
                    })
                    .await?;

                self.cur_block_size = 0;
                self.cur_proposal_timeout = None;
                if let DiemConsensusMsg::Proposal { block, payload, .. } = proposal {
                    self.blk_pool.insert(blk_id, (block, payload)); // avoid extra copy
                } else {
                    unreachable!()
                }

                self.make_vote(blk_id, round, parent_id, parent_round, grand_parent_id)
                    .await?;
            }
        }

        // make vote for current round
        if let Some((blk_id, round, parent_id, parent_round, grandparent_id)) =
            self.pending_votes.remove(&PendingVoteDep::Round {
                round: self.current_round,
            })
        {
            if !self.blk_seen(&parent_id) {
                self.pending_votes.insert(
                    PendingVoteDep::Parent { blk_id: parent_id },
                    (blk_id, round, parent_id, parent_round, grandparent_id),
                );
            } else {
                self.make_vote(blk_id, round, parent_id, parent_round, grandparent_id)
                    .await?;
            }
        }

        // TODO: handle vote timeout

        Ok(())
    }

    async fn handle_peer_msg(&mut self, src: NodeId, content: Vec<u8>) -> Result<(), CopycatError> {
        let msg: DiemConsensusMsg = match bincode::deserialize(&content) {
            Ok(msg) => msg,
            Err(e) => {
                pf_error!(self.id; "unrecognized msg from {}: {:?}", src, e);
                return Ok(());
            }
        };

        match msg {
            DiemConsensusMsg::Proposal {
                block,
                last_round_tc,
                high_commit_qc,
                signature,
                payload,
            } => {
                let blk_id = block.compute_id(&payload)?;
                let blk_recv_time = Instant::now();
                let blk_send_time = blk_recv_time - *PROPOSAL_SEND_TIME.get(&blk_id).unwrap();
                self.blk_send_times.push(blk_send_time.as_secs_f64());

                // validate proposal
                let qc_valid = if block.qc == *GENESIS_QC {
                    true
                } else {
                    let (valid, verify_time) = self.validate_qc_signatures(&block.qc)?;
                    self.delay.process_illusion(verify_time).await;
                    if valid {
                        self.process_certificate_qc(&block.qc).await?;
                    }
                    valid
                };

                let commit_qc_valid = if high_commit_qc == *GENESIS_QC {
                    true
                } else {
                    let (valid, verify_time) = self.validate_qc_signatures(&high_commit_qc)?;
                    self.delay.process_illusion(verify_time).await;
                    if valid {
                        self.process_certificate_qc(&high_commit_qc).await?;
                    }
                    valid
                };

                let tc_valid = {
                    let well_formed = if let Some(tc) = &last_round_tc {
                        tc.tmo_high_qc_rounds.len() == tc.tmo_signatures.len()
                            && tc.tmo_signatures.len() >= self.all_nodes.len() - self.num_faulty
                    } else {
                        true
                    };
                    if well_formed {
                        let (valid, verify_time) = self.validate_tc_signatures(&last_round_tc)?;
                        self.delay.process_illusion(verify_time).await;
                        if valid {
                            self.process_certificate_tc(&last_round_tc)?;
                        }
                        valid
                    } else {
                        false
                    }
                };

                let signature_valid = match self.peer_pks.get(&block.proposer) {
                    Some(pk) => {
                        let serialized = bincode::serialize(&blk_id)?;
                        let (valid, verify_time) =
                            self.signature_scheme.verify(&pk, &serialized, &signature)?;
                        self.delay.process_illusion(verify_time).await;
                        valid
                    }
                    None => false,
                };

                // validate CoAs
                let mut payload_valid = true;
                for coa in payload.iter() {
                    let (valid, dur) = coa.validate(self.threshold_signature.as_ref())?;
                    self.delay.process_illusion(dur).await;
                    if !valid {
                        payload_valid = false;
                        break;
                    }
                }

                if !(qc_valid && commit_qc_valid && tc_valid && signature_valid && payload_valid) {
                    return Ok(()); // invalid proposal, do nothing
                }

                let round = block.round;
                let leader = Self::get_leader(round, &self.all_nodes);
                if block.proposer != leader || src != leader {
                    return Ok(()); // proposal not from the leader, do nothing
                }

                // decide if we should vote for proposal
                let qc_round = block.qc.vote_info.round;
                let parent_id = block.qc.vote_info.blk_id;
                let grandparent_id = block.qc.vote_info.parent_id;

                // TODO: insert to committed pool right away for now since all blocks will commit
                for batch in payload.iter() {
                    self.committed_pool.insert((batch.sender, batch.round));
                }

                self.blk_pool.insert(blk_id, (block, payload));
                self.process_waiting_actions(blk_id, parent_id).await?;

                if round < self.current_round
                    || round <= self.highest_vote_round
                    || round <= qc_round
                {
                    return Ok(());
                }

                let qc_safe = qc_round + 1 == round;
                let tc_safe = match last_round_tc {
                    Some(tc) => {
                        tc.round + 1 == round
                            && qc_round > *tc.tmo_high_qc_rounds.values().max().unwrap()
                    }
                    None => false,
                };

                self.blk_validate_times
                    .push((Instant::now() - blk_recv_time).as_secs_f64());

                if qc_safe || tc_safe {
                    // make vote
                    if round > self.current_round {
                        self.pending_votes.insert(
                            PendingVoteDep::Round { round: round },
                            (blk_id, round, parent_id, qc_round, grandparent_id),
                        );
                    } else if !self.blk_seen(&parent_id) {
                        self.pending_votes.insert(
                            PendingVoteDep::Parent { blk_id: parent_id },
                            (blk_id, round, parent_id, qc_round, grandparent_id),
                        );
                    } else {
                        self.make_vote(blk_id, round, parent_id, qc_round, grandparent_id)
                            .await?;
                    }
                }
            }
            DiemConsensusMsg::Vote {
                vote_info,
                ledger_commit_info,
                high_commit_qc,
                sender,
                signature,
            } => {
                let vote_recv_time = Instant::now();
                let vote_send_time =
                    vote_recv_time - *VOTE_SEND_TIME.get(&(vote_info.blk_id, sender)).unwrap();
                self.vote_send_times.push(vote_send_time.as_secs_f64());
                // validate vote
                let commit_qc_valid = if high_commit_qc == *GENESIS_QC {
                    true
                } else {
                    let (valid, verify_time) = self.validate_qc_signatures(&high_commit_qc)?;
                    self.delay.process_illusion(verify_time).await;
                    if valid {
                        self.process_certificate_qc(&high_commit_qc).await?;
                    }
                    valid
                };

                if !commit_qc_valid || src != sender {
                    return Ok(());
                }

                self.record_vote(vote_info, ledger_commit_info, sender, signature)
                    .await?;
                let vote_validate_time = Instant::now() - vote_recv_time;
                self.vote_validate_times
                    .push(vote_validate_time.as_secs_f64());
            }
            DiemConsensusMsg::FetchBlockReq { blk_id } => {
                let (blk, payload) = match self.blk_pool.get(&blk_id) {
                    Some(blk) => blk,
                    None => return Ok(()), // I don't have the block either, do nothing
                };

                // TODO: for now, only let the proposer respond
                if blk.proposer != self.id {
                    return Ok(());
                }

                // TODO: use pointer types to avoid copying
                let msg = DiemConsensusMsg::FetchBlockResp {
                    blk: blk.clone(),
                    payload: payload.clone(),
                };
                self.peer_messenger
                    .send(
                        src,
                        MsgType::ConsensusMsg {
                            msg: bincode::serialize(&msg)?,
                        },
                    )
                    .await?;
            }
            DiemConsensusMsg::FetchBlockResp { blk, payload } => {
                let blk_id = blk.compute_id(&payload)?;
                let parent_id = blk.qc.vote_info.blk_id;

                // already seen block, do nothing
                if self.blk_pool.contains_key(&blk_id) {
                    return Ok(());
                }

                // validate block
                if blk.qc != *GENESIS_QC {
                    let (valid, verify_time) = self.validate_qc_signatures(&blk.qc)?;
                    self.delay.process_illusion(verify_time).await;
                    if valid {
                        self.process_certificate_qc(&blk.qc).await?;
                    } else {
                        return Ok(());
                    }
                };

                self.queried_blks.remove(&blk_id);
                self.blk_pool.insert(blk_id, (blk, payload)); // this is safe assuming hash collision resistance
                self.process_waiting_actions(blk_id, parent_id).await?;
            }
            DiemConsensusMsg::FetchBatchReq { sender, round } => {
                // TODO: for now, only let the sender respond
                if sender != self.id {
                    return Ok(());
                }

                let (batch, _) = match self.quorum_store.get(&(sender, round)) {
                    Some(batch) => batch.clone(),
                    None => return Ok(()), // I don't have the batch either, do nothing - this should not happen
                };

                let msg = DiemConsensusMsg::FetchBatchResp { batch };
                self.peer_messenger
                    .send(
                        src,
                        MsgType::ConsensusMsg {
                            msg: bincode::serialize(&msg)?, // TODO: this will lead to an extra copy of block
                        },
                    )
                    .await?;
            }
            DiemConsensusMsg::FetchBatchResp { batch } => {
                let (sender, round, merkle_root) = match &batch.header {
                    BlockHeader::Aptos {
                        sender,
                        round,
                        merkle_root,
                        ..
                    } => (sender, round, merkle_root),
                    _ => unreachable!(),
                };

                if self.quorum_store.contains_key(&(*sender, *round)) {
                    return Ok(());
                }

                // validate block content - signature and etc have been verified by at least N-2f correct nodes
                // this check ensures that batch content matches with merkle root and hence with CoA
                let mut merkle_tree = DummyMerkleTree::new();
                let dur = merkle_tree.append(batch.txns.len())?;
                self.delay.process_illusion(dur).await;
                if !merkle_tree.verify_root(merkle_root)? {
                    return Ok(());
                }

                // validate against CoA
                let coa = match self.queried_batchs.get(&(*sender, *round)) {
                    Some(coa) => coa,
                    None => return Ok(()),
                };

                let digest = sha256(&batch.header)?;
                let content = &(sender, round, digest);
                let serialized = bincode::serialize(&content)?;
                let (valid, dur) = self
                    .threshold_signature
                    .verify(&serialized, &coa.signature)?;
                self.delay.process_illusion(dur).await;
                if !valid {
                    // block does not match CoA
                    return Ok(());
                }

                let ctx = Arc::new(BlkCtx::from_blk(&batch)?);
                self.queried_batchs.remove(&(*sender, *round));
                self.quorum_store.insert((*sender, *round), (batch, ctx));
            }
        }
        Ok(())
    }

    fn report(&mut self) {
        let blk_build_times = std::mem::take(&mut self.blk_build_times);
        let blk_send_times = std::mem::take(&mut self.blk_send_times);
        let blk_validate_times = std::mem::take(&mut self.blk_validate_times);
        let vote_recv_times = std::mem::take(&mut self.vote_send_times);
        let vote_validate_times = std::mem::take(&mut self.vote_validate_times);
        let round_time = std::mem::take(&mut self.round_latency);

        let avg_blk_build_times =
            blk_build_times.iter().sum::<f64>() / blk_build_times.len() as f64;
        let avg_blk_validation_times =
            blk_validate_times.iter().sum::<f64>() / blk_validate_times.len() as f64;
        let avg_vote_validation_time =
            vote_validate_times.iter().sum::<f64>() / vote_validate_times.len() as f64;
        let avg_round_latency = round_time.iter().sum::<f64>() / round_time.len() as f64;

        pf_info!(self.id; "avg_blk_build_times: {}, avg_blk_validation_times: {}, avg_vote_validation_time: {}, avg_round_latency: {}", 
                                avg_blk_build_times, avg_blk_validation_times, avg_vote_validation_time, avg_round_latency);
        pf_info!(self.id; "blk_send_times: {:?}", blk_send_times);
        pf_info!(self.id; "vote_recv_times: {:?}", vote_recv_times);
    }
}
