use get_size::GetSize;
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};

use std::{
    fmt::{Debug, Formatter},
    sync::Arc,
};

use super::crypto::{sha256, threshold_signature::SignComb, Signature};
use crate::protocol::transaction::Txn;
use crate::NodeId;
use crate::{protocol::crypto::Hash, CopycatError};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BlockHeader {
    Dummy,
    Bitcoin {
        proposer: NodeId,
        parent_id: Hash, // vec![] if first block of chain
        merkle_root: Hash,
        nonce: u32, // https://en.bitcoin.it/wiki/Nonce
    },
    Avalanche {
        // only include data to uniquely identify each rounds of vote
        proposer: NodeId,
        id: u64,
        // for querying depending txns that current node has not seen yet
        // new batches have depth 0, and the txns parents would have depth 1, and their parents would have depth 2, etc
        depth: usize,
    },
    ChainReplication {
        blk_id: Hash,
    },
    Diem {
        proposer: NodeId,
        round: u64,
        parent_id: Hash,
        parent_round: u64,
        payload_hash: Hash,
        state_id: Hash,
        qc: SignComb,
        proposer_signature: Signature,
    },
}

impl BlockHeader {
    pub fn compute_id(&self) -> Result<Hash, CopycatError> {
        match self {
            BlockHeader::Dummy => {
                let mut rng = thread_rng();
                let mut rand = [0u8; 32];
                rng.fill(&mut rand);
                Ok(Hash::from_little_endian(&rand))
            }
            BlockHeader::Avalanche {
                proposer,
                id,
                depth,
            } => {
                let raw = [0u64, *depth as u64, *proposer, *id];
                Ok(Hash { 0: raw })
            }
            BlockHeader::Bitcoin { .. } => {
                let serialized_header = bincode::serialize(self)?;
                Ok(sha256(&serialized_header)?)
            }
            BlockHeader::ChainReplication { blk_id } => Ok(*blk_id),
            BlockHeader::Diem {
                proposer,
                round,
                payload_hash,
                parent_id,
                qc,
                ..
            } => {
                let serialized =
                    bincode::serialize(&(proposer, round, payload_hash, parent_id, qc))?;
                Ok(sha256(&serialized)?)
            }
        }
    }
}

// TODO: for better accuracy, we should implement GetSize manually so that message size
// matches the size after marshalling.
impl GetSize for BlockHeader {}

#[derive(Clone, Serialize, Deserialize, GetSize)]
pub struct Block {
    pub header: BlockHeader,
    pub txns: Vec<Arc<Txn>>,
}

impl Debug for Block {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        self.header.fmt(f)
    }
}
