use crate::{
    protocol::crypto::{sha256, threshold_signature::SignComb, Hash, Signature},
    CopycatError, NodeId,
};

use mailbox_client::{MailboxError, SizedMsg};
use primitive_types::U256;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use lazy_static::lazy_static;

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct VoteInfo {
    pub blk_id: Hash,
    pub round: u64,
    pub parent_id: Hash,
    pub parent_round: u64,
    pub exec_state_hash: Hash,
}

impl SizedMsg for VoteInfo {
    fn size(&self) -> Result<usize, MailboxError> {
        let size = 32 // blk_id
            + 8              // round
            + 32             // parent_id
            + 8              // parent_round
            + 32; // exec_state_hash
        Ok(size)
    }
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

impl SizedMsg for LedgerCommitInfo {
    fn size(&self) -> Result<usize, MailboxError> {
        let commit_state_id_size = match &self.commit_state_id {
            Some(_) => 1 + 32,
            None => 1,
        };
        let size = commit_state_id_size + 32; // vote_info_hash
        Ok(size)
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

impl SizedMsg for QuorumCert {
    fn size(&self) -> Result<usize, MailboxError> {
        let size = self.vote_info.size()? + self.commit_info.size()? + self.signatures.len()
            + 8 // author
            + self.author_signature.len(); // author_signature
        Ok(size)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiemBlock {
    pub proposer: NodeId,
    pub round: u64,
    pub state_id: Hash,
    pub qc: QuorumCert,
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

impl SizedMsg for DiemBlock {
    fn size(&self) -> Result<usize, MailboxError> {
        let size = 8 + 8 + 32 + self.qc.size()?;
        Ok(size)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimeoutInfo {
    pub round: u64,
    pub high_qc: QuorumCert,
    pub sender: NodeId,
    pub signature: Signature,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimeCert {
    pub round: u64,
    pub tmo_high_qc_rounds: HashMap<NodeId, u64>,
    pub tmo_signatures: HashMap<NodeId, Signature>,
}

impl SizedMsg for TimeCert {
    fn size(&self) -> Result<usize, MailboxError> {
        assert!(self.tmo_high_qc_rounds.len() == self.tmo_signatures.len());
        let signature_size = self
            .tmo_signatures
            .iter()
            .map(|(_, sig)| sig.len())
            .sum::<usize>();
        let size = 8 + self.tmo_high_qc_rounds.len() * (8 + 8) + signature_size;
        Ok(size)
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
