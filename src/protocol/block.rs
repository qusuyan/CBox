use get_size::GetSize;
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};

use std::{
    fmt::{Debug, Formatter},
    sync::Arc,
};

use super::types::diem::QuorumCert;
use super::{
    crypto::{sha256, Signature},
    types::diem::{DiemBlock, TimeCert},
};
use crate::NodeId;
use crate::{context::TxnCtx, protocol::transaction::Txn};
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
        merkle_root: Hash,
        // for querying depending txns that current node has not seen yet
        // new batches have depth 0, and the txns parents would have depth 1, and their parents would have depth 2, etc
        depth: usize,
        signature: Signature,
    },
    ChainReplication {
        blk_id: Hash,
    },
    Diem {
        // block data
        block: DiemBlock,
        // proposal message
        last_round_tc: Option<TimeCert>,
        high_commit_qc: QuorumCert,
        signature: Signature,
    },
}

// TODO: for better accuracy, we should implement GetSize manually so that message size
// matches the size after marshalling.
impl GetSize for BlockHeader {}

#[derive(Clone, Serialize, Deserialize, GetSize)]
pub struct Block {
    pub header: BlockHeader,
    pub txns: Vec<Arc<Txn>>,
}

impl Block {
    pub fn compute_id(
        header: &BlockHeader,
        txn_ctx: &Vec<Arc<TxnCtx>>,
    ) -> Result<Hash, CopycatError> {
        match &header {
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
                ..
            } => {
                let raw = [0u64, *depth as u64, *proposer, *id];
                Ok(Hash { 0: raw })
            }
            BlockHeader::Bitcoin { .. } => Ok(sha256(&header)?),
            BlockHeader::ChainReplication { blk_id } => Ok(*blk_id),
            BlockHeader::Diem { block, .. } => block.compute_id(txn_ctx),
        }
    }
}

impl Debug for Block {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        self.header.fmt(f)
    }
}
