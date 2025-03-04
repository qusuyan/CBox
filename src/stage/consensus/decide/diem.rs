use super::Decision;
use crate::config::DiemConfig;
use crate::context::BlkCtx;
use crate::peers::PeerMessenger;
use crate::protocol::block::{Block, BlockHeader};
use crate::protocol::crypto::signature::P2PSignature;
use crate::protocol::crypto::threshold_signature::{SignPart, ThresholdSignature};
use crate::protocol::crypto::{sha256, Hash, PrivKey, PubKey};
use crate::protocol::types::diem::{
    LedgerCommitInfo, QuorumCert, VoteInfo, GENESIS_QC, GENESIS_VOTE_INFO,
};
use crate::protocol::MsgType;
use crate::stage::pacemaker::diem::DiemPacemaker;
use crate::stage::process_illusion;
use crate::transaction::Txn;
use crate::{CopycatError, NodeId, SignatureScheme};

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{mpsc, Notify};

use async_trait::async_trait;
use atomic_float::AtomicF64;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VoteMsg {
    vote_info: VoteInfo,
    ledger_commit_info: LedgerCommitInfo,
    high_commit_qc: QuorumCert,
    sender: NodeId,
    signature: SignPart,
}

pub struct DiemDecision {
    id: NodeId,
    proposal_timeout_secs: f64,
    vote_timeout_secs: f64,
    blk_pool: HashMap<Hash, (Arc<Block>, Arc<BlkCtx>, u64)>,
    commit_queue: HashMap<u64, Hash>,
    next_commit_height: u64,
    // consensus
    highest_vote_round: u64,
    highest_qc_round: u64,
    high_commit_qc: QuorumCert,
    state_id_map: HashMap<Hash, Hash>,
    pending_votes: HashMap<Hash, HashMap<NodeId, SignPart>>,
    closed_votes: HashSet<Hash>,
    // p2p communicaton
    all_nodes: Vec<NodeId>,
    peer_messenger: Arc<PeerMessenger>,
    signature_scheme: SignatureScheme,
    peer_pks: HashMap<NodeId, PubKey>,
    sk: PrivKey,
    threshold_signature: Arc<dyn ThresholdSignature>,
    //
    pmaker_feedback_send: mpsc::Sender<Vec<u8>>,
    delay: Arc<AtomicF64>,
    _notify: Notify,
}

impl DiemDecision {
    pub fn new(
        id: NodeId,
        p2p_signature: P2PSignature,
        threshold_signature: Arc<dyn ThresholdSignature>,
        config: DiemConfig,
        peer_messenger: Arc<PeerMessenger>,
        pmaker_feedback_send: mpsc::Sender<Vec<u8>>,
        delay: Arc<AtomicF64>,
    ) -> Self {
        let (signature_scheme, peer_pks, sk) = p2p_signature;
        let mut state_id_map = HashMap::new();
        // add genesis block
        state_id_map.insert(GENESIS_VOTE_INFO.blk_id, GENESIS_VOTE_INFO.exec_state_hash);

        let mut all_nodes: Vec<NodeId> = peer_pks.keys().cloned().collect();
        all_nodes.sort();

        // let mut committed = HashSet::new();
        // committed.insert(GENESIS_VOTE_INFO.blk_id);
        // committed.insert(GENESIS_VOTE_INFO.parent_id);

        let genesis_qc = GENESIS_QC.clone();

        Self {
            id,
            proposal_timeout_secs: config.proposal_timeout_secs,
            vote_timeout_secs: config.vote_timeout_secs,
            blk_pool: HashMap::new(),
            commit_queue: HashMap::new(),
            next_commit_height: 2,
            highest_qc_round: 0,
            highest_vote_round: 0,
            high_commit_qc: genesis_qc,
            state_id_map,
            pending_votes: HashMap::new(),
            closed_votes: HashSet::new(),
            peer_messenger,
            all_nodes,
            signature_scheme,
            peer_pks,
            sk,
            threshold_signature,
            pmaker_feedback_send,
            delay,
            _notify: Notify::new(),
        }
    }

    async fn process_certificate_qc(&mut self, qc: &QuorumCert) -> Result<(), CopycatError> {
        if let Some(_id) = qc.commit_info.commit_state_id {
            let commit_blk_id = qc.vote_info.parent_id;
            let commit_height = match self.get_height(commit_blk_id) {
                Ok(height) => height,
                Err(_) => return Ok(()), // missing some dependencies, will commit later
            };
            let mut cur_height = commit_height;
            let mut cur_blk_id = commit_blk_id;
            while cur_height >= self.next_commit_height {
                self.commit_queue.insert(cur_height, cur_blk_id);
                cur_height -= 1;
                let (blk, _, _) = self.blk_pool.get(&cur_blk_id).unwrap();
                let diem_blk = match &blk.header {
                    BlockHeader::Diem { block, .. } => block,
                    _ => unreachable!(),
                };
                cur_blk_id = diem_blk.qc.vote_info.blk_id;
            }
            if qc.vote_info.round > self.high_commit_qc.vote_info.round {
                self.high_commit_qc = qc.clone();
            }
        }

        Ok(())
    }

    async fn add_vote(&mut self, vote_msg: VoteMsg) -> Result<Option<QuorumCert>, CopycatError> {
        let vote_id = sha256(&vote_msg.ledger_commit_info)?;
        if self.closed_votes.contains(&vote_id) {
            return Ok(None);
        }

        let votes = self
            .pending_votes
            .entry(vote_id)
            .or_insert_with(HashMap::new);
        votes.insert(vote_msg.sender, vote_msg.signature);

        let ledger_commit_info_serialized = bincode::serialize(&vote_msg.ledger_commit_info)?;
        let signcomb = match self
            .threshold_signature
            .aggregate(&ledger_commit_info_serialized, votes)
        {
            Ok((signature, dur)) => {
                process_illusion(dur, &self.delay).await;
                signature
            }
            Err(e) => {
                pf_warn!(self.id; "failed to aggregate votes: {:?}", e);
                self.pending_votes.remove(&vote_id);
                self.closed_votes.insert(vote_id);
                return Ok(None);
            }
        };

        let qc = match signcomb {
            Some(signcomb) => {
                let (author_signature, dur) =
                    self.signature_scheme.sign(&self.sk, &signcomb).unwrap();
                process_illusion(dur, &self.delay).await;
                self.pending_votes.remove(&vote_id);
                self.closed_votes.insert(vote_id);
                Some(QuorumCert {
                    vote_info: vote_msg.vote_info,
                    commit_info: vote_msg.ledger_commit_info,
                    signatures: signcomb,
                    author: self.id,
                    author_signature,
                })
            }
            None => None,
        };

        Ok(qc)
    }

    fn get_height(&self, blk_id: Hash) -> Result<u64, CopycatError> {
        if blk_id == GENESIS_VOTE_INFO.parent_id {
            Ok(0)
        } else if blk_id == GENESIS_VOTE_INFO.blk_id {
            Ok(1)
        } else {
            match self.blk_pool.get(&blk_id) {
                Some((_, _, height)) => Ok(*height),
                None => Err(CopycatError(format!("Unknown block {}", blk_id))),
            }
        }
    }
}

#[async_trait]
impl Decision for DiemDecision {
    async fn new_tail(
        &mut self,
        _src: NodeId,
        new_tail: Vec<(Arc<Block>, Arc<BlkCtx>)>,
    ) -> Result<(), CopycatError> {
        for (blk, ctx) in new_tail {
            let mut check_time_secs = 0f64;
            let (diem_blk, last_tc, high_commit_qc) = match &blk.header {
                BlockHeader::Diem {
                    block,
                    last_round_tc,
                    high_commit_qc,
                    ..
                } => (block, last_round_tc, high_commit_qc),
                _ => continue,
            };

            let parent_blk_id = diem_blk.qc.vote_info.blk_id;
            let parent_height = self.get_height(parent_blk_id)?;
            self.blk_pool
                .insert(ctx.id, (blk.clone(), ctx.clone(), parent_height + 1));
            let qc_round = diem_blk.qc.vote_info.round;

            self.state_id_map.insert(ctx.id, diem_blk.state_id);

            // commit grandparent block (diem_blk.qc.vote_info.parent_id)
            self.process_certificate_qc(&diem_blk.qc).await?;
            self.process_certificate_qc(&high_commit_qc).await?;

            // safe_to_vote(b.round, qc_round, last_tc)
            // one vote per round                         block must extend a smaller round
            if diem_blk.round <= self.highest_qc_round || diem_blk.round <= qc_round {
                continue;
            }

            let consecutive = diem_blk.round == qc_round + 1;
            let safe_to_extend = last_tc
                .as_ref()
                .map(|tc| {
                    (diem_blk.round == tc.round + 1)    // block generated in response to last tc
                        // benign leader will choose round to be greater than all valid timeout messages
                        && (qc_round >= *tc.tmo_high_qc_rounds.values().max().unwrap())
                })
                .unwrap_or(false);

            if !(consecutive || safe_to_extend) {
                continue;
            }

            // update_highest_qc_round
            if qc_round > self.highest_qc_round {
                self.highest_qc_round = qc_round;
            }

            // increase_highest_vote_round
            if diem_blk.round > self.highest_vote_round {
                self.highest_vote_round = diem_blk.round;
            }

            // build vote message
            let vote_info = VoteInfo {
                blk_id: ctx.id,
                round: diem_blk.round,
                parent_id: diem_blk.qc.vote_info.blk_id,
                parent_round: qc_round,
                exec_state_hash: diem_blk.state_id,
            };
            let commit_state_id = if consecutive {
                let state_id = self
                    .state_id_map
                    .get(&diem_blk.qc.vote_info.blk_id)
                    .unwrap();
                Some(*state_id)
            } else {
                None
            };
            let vote_info_hash = sha256(&vote_info)?;
            let ledger_commit_info = LedgerCommitInfo {
                commit_state_id,
                vote_info_hash,
            };

            let ledger_commit_info_serialized = bincode::serialize(&ledger_commit_info)?;
            let (signature, dur) = self
                .threshold_signature
                .sign(&ledger_commit_info_serialized)?;
            check_time_secs += dur;

            let vote_msg = VoteMsg {
                vote_info,
                ledger_commit_info,
                high_commit_qc: self.high_commit_qc.clone(),
                sender: self.id,
                signature,
            };

            process_illusion(check_time_secs, &self.delay).await;

            let next_leader = DiemPacemaker::get_leader(diem_blk.round + 1, &self.all_nodes);
            pf_debug!(self.id; "Sending vote to node {}: {:?}", next_leader, vote_msg);
            if next_leader == self.id {
                let qc = self.add_vote(vote_msg).await?;
                if let Some(qc) = qc {
                    self.process_certificate_qc(&qc).await?;
                    self.pmaker_feedback_send
                        .send(bincode::serialize(&qc)?)
                        .await?;
                }
            } else {
                // send vote message to next leader
                let msg = bincode::serialize(&vote_msg)?;
                self.peer_messenger
                    .send(next_leader, MsgType::ConsensusMsg { msg })
                    .await?;
            }
        }

        Ok(())
    }

    async fn commit_ready(&self) -> Result<(), CopycatError> {
        loop {
            if self.commit_queue.contains_key(&self.next_commit_height) {
                break Ok(());
            }
            self._notify.notified().await;
        }
    }

    async fn next_to_commit(&mut self) -> Result<(u64, Vec<Arc<Txn>>), CopycatError> {
        let blk_id = self.commit_queue.remove(&self.next_commit_height).unwrap();
        self.next_commit_height += 1;
        let (blk, _, height) = self.blk_pool.get(&blk_id).unwrap();
        let txns = blk.txns.clone();
        Ok((*height, txns))
    }

    async fn timeout(&self) -> Result<(), CopycatError> {
        // TODO: add timeout
        self._notify.notified().await;
        Ok(())
    }

    async fn handle_timeout(&mut self) -> Result<(), CopycatError> {
        todo!()
    }

    async fn handle_peer_msg(&mut self, src: NodeId, content: Vec<u8>) -> Result<(), CopycatError> {
        let vote_msg: VoteMsg = bincode::deserialize(&content)?;
        if src != vote_msg.sender {
            return Ok(());
        }
        let qc = self.add_vote(vote_msg).await?;
        if let Some(qc) = qc {
            self.process_certificate_qc(&qc).await?;
            self.pmaker_feedback_send
                .send(bincode::serialize(&qc)?)
                .await?;
        }
        Ok(())
    }

    fn report(&mut self) {}
}
