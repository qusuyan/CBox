use crate::{
    context::TxnCtx,
    protocol::crypto::{sha256, threshold_signature::SignComb, Hash, Signature},
    CopycatError, NodeId,
};

use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VoteInfo {
    pub blk_id: Hash,
    pub round: u64,
    pub parent_id: Hash,
    pub parent_round: u64,
    pub exec_state_hash: Hash,
}

impl VoteInfo {
    pub fn compute_id(&self) -> Result<Hash, CopycatError> {
        let serialized = bincode::serialize(self)?;
        sha256(&serialized)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LedgerCommitInfo {
    pub commit_state_id: Option<Hash>,
    pub vote_info_hash: Hash,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuorumCert {
    pub vote_info: VoteInfo,
    pub commit_info: LedgerCommitInfo,
    pub signatures: SignComb,
    pub author: NodeId,
    pub author_signature: Signature,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimeCert {
    pub round: u64,
    pub tmo_high_qc_rounds: Vec<u64>,
    pub tmo_signatures: SignComb,
}
