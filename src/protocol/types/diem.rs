use crate::{
    context::TxnCtx,
    protocol::crypto::{sha256, threshold_signature::SignComb, Hash, Signature},
    CopycatError, NodeId,
};

use get_size::GetSize;
use primitive_types::U256;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};

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

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, GetSize)]
pub struct LedgerCommitInfo {
    pub commit_state_id: Option<Hash>,
    pub vote_info_hash: Hash,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, GetSize)]
pub struct QuorumCert {
    pub vote_info: VoteInfo,
    pub commit_info: LedgerCommitInfo,
    pub signatures: SignComb,
    pub author: NodeId,
    pub author_signature: Signature,
}

#[derive(Clone, Debug, Serialize, Deserialize, GetSize)]
pub struct DiemBlock {
    pub proposer: NodeId,
    pub round: u64,
    pub state_id: Hash,
    pub qc: QuorumCert,
}

impl DiemBlock {
    pub fn compute_id(&self, txn_ctxs: &Vec<Arc<TxnCtx>>) -> Result<Hash, CopycatError> {
        let serialized = bincode::serialize(&(
            self.proposer,
            self.round,
            txn_ctxs,
            self.qc.vote_info.blk_id,
            self.qc.signatures.clone(),
        ))?;
        sha256(&serialized)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, GetSize)]
pub struct TimeoutInfo {
    pub round: u64,
    pub high_qc: QuorumCert,
    pub sender: NodeId,
    pub signature: Signature,
}

#[derive(Clone, Debug, Serialize, Deserialize, GetSize)]
pub struct TimeCert {
    pub round: u64,
    pub tmo_high_qc_rounds: HashMap<NodeId, u64>,
    pub tmo_signatures: HashMap<NodeId, Signature>,
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
