use crate::{
    protocol::crypto::{sha256, threshold_signature::SignComb, Hash, Signature},
    CopycatError, NodeId,
};

use get_size::GetSize;
use primitive_types::U256;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use lazy_static::lazy_static;

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, GetSize)]
pub struct VoteInfo {
    pub blk_id: Hash,
    pub round: u64,
    pub parent_id: Hash,
    pub parent_round: u64,
    pub exec_state_hash: Hash,
}

impl VoteInfo {
    pub fn compute_id(&self) -> Result<Hash, CopycatError> {
        sha256(&self)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct LedgerCommitInfo {
    pub commit_state_id: Option<Hash>,
    pub vote_info_hash: Hash,
}

impl GetSize for LedgerCommitInfo {
    fn get_size(&self) -> usize {
        let commit_id_size = match self.commit_state_id {
            Some(id) => 1 + id.get_size(),
            None => 1,
        };
        commit_id_size + 32
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct QuorumCert {
    pub vote_info: VoteInfo,
    pub commit_info: LedgerCommitInfo,
    pub signatures: SignComb,
    pub author: NodeId,
    pub author_signature: Signature,
}

impl GetSize for QuorumCert {
    fn get_size(&self) -> usize {
        self.vote_info.get_size()
            + self.commit_info.get_size()
            + self.signatures.len()
            + 8 // NodeId is 8 bytes
            + self.author_signature.len()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiemBlock {
    pub proposer: NodeId,
    pub round: u64,
    pub state_id: Hash,
    pub qc: QuorumCert,
}

impl GetSize for DiemBlock {
    fn get_size(&self) -> usize {
        48 + self.qc.get_size()
    }
}

impl DiemBlock {
    pub fn compute_id<T: Serialize>(&self, txn_ctxs: &T) -> Result<Hash, CopycatError> {
        let input = (
            self.proposer,
            self.round,
            txn_ctxs,
            self.qc.vote_info.blk_id,
            self.qc.signatures.clone(),
        );
        sha256(&input)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimeoutInfo {
    pub round: u64,
    pub high_qc: QuorumCert,
    pub sender: NodeId,
    pub signature: Signature,
}

impl GetSize for TimeoutInfo {
    fn get_size(&self) -> usize {
        16 + self.high_qc.get_size() + self.signature.len()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimeCert {
    pub round: u64,
    pub tmo_high_qc_rounds: HashMap<NodeId, u64>,
    pub tmo_signatures: HashMap<NodeId, Signature>,
}

impl GetSize for TimeCert {
    fn get_size(&self) -> usize {
        8 + self.tmo_high_qc_rounds.len() * (8 + 8)
            + self
                .tmo_signatures
                .iter()
                .map(|(_, signature)| 8 + signature.len())
                .sum::<usize>()
    }
}

lazy_static! {
    pub static ref GENESIS_VOTE_INFO: VoteInfo = VoteInfo {
        blk_id: Hash(U256::one()),
        round: 1,
        parent_id: Hash(U256::zero()),
        parent_round: 0,
        exec_state_hash: Hash(U256::zero()),
    };
    pub static ref GENESIS_COMMIT_INFO: LedgerCommitInfo = LedgerCommitInfo {
        commit_state_id: None,
        vote_info_hash: GENESIS_VOTE_INFO.compute_id().unwrap(),
    };
    pub static ref GENESIS_QC: QuorumCert = QuorumCert {
        vote_info: GENESIS_VOTE_INFO.clone(),
        commit_info: GENESIS_COMMIT_INFO.clone(),
        signatures: SignComb::new(),
        author: 0,
        author_signature: Signature::new(),
    };
}
