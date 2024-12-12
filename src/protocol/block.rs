use get_size::GetSize;
use serde::{Deserialize, Serialize};

use std::{
    fmt::{Debug, Formatter},
    sync::Arc,
};

use super::crypto::sha256;
use crate::protocol::transaction::Txn;
use crate::NodeId;
use crate::{protocol::crypto::Hash, CopycatError};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BlockHeader {
    Dummy,
    Bitcoin {
        proposer: NodeId,
        prev_hash: Hash, // vec![] if first block of chain
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
}

impl BlockHeader {
    pub fn compute_id(&self) -> Result<Hash, CopycatError> {
        match self {
            BlockHeader::Dummy | BlockHeader::Avalanche { .. } => Ok(Hash::zero()),
            BlockHeader::Bitcoin { .. } => {
                let serialized_header = bincode::serialize(self)?;
                Ok(sha256(&serialized_header)?)
            }
            BlockHeader::ChainReplication { blk_id } => Ok(*blk_id),
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
