use mailbox_client::{MailboxError, SizedMsg};
use primitive_types::U256;
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};

use std::{
    fmt::{Debug, Formatter},
    sync::Arc,
};

use super::types::{aptos::CoA, diem::QuorumCert};
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
    Aptos {
        sender: NodeId,
        round: u64,
        certificates: Vec<CoA>,
        merkle_root: Hash,
        signature: Signature,
    },
}

impl SizedMsg for BlockHeader {
    fn size(&self) -> Result<usize, MailboxError> {
        match self {
            BlockHeader::Dummy => Ok(0),
            BlockHeader::Bitcoin { .. } => Ok(32 + 32 + 4 + 8),
            BlockHeader::Avalanche { signature, .. } => Ok(8 + 8 + 32 + 8 + signature.len()),
            BlockHeader::ChainReplication { .. } => Ok(32),
            BlockHeader::Diem {
                block,
                last_round_tc,
                high_commit_qc,
                signature,
            } => {
                let tc_size = match last_round_tc {
                    Some(tc) => 1 + tc.size()?,
                    None => 1,
                };
                Ok(block.size()? + tc_size + high_commit_qc.size()? + signature.len())
            }
            BlockHeader::Aptos {
                certificates,
                signature,
                ..
            } => Ok(8 + 8 + certificates.size()? + 32 + signature.len()),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
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
                Ok(Hash(U256::from_little_endian(&rand)))
            }
            BlockHeader::Avalanche {
                proposer,
                id,
                depth,
                ..
            } => {
                let raw = [0u64, *depth as u64, *proposer, *id];
                Ok(Hash(U256(raw)))
            }
            BlockHeader::Bitcoin { .. } => Ok(sha256(&header)?),
            BlockHeader::ChainReplication { blk_id } => Ok(*blk_id),
            BlockHeader::Diem { block, .. } => block.compute_id(txn_ctx),
            BlockHeader::Aptos { sender, round, .. } => {
                let raw = [0u64, 0u64, *sender, *round];
                Ok(Hash(U256(raw)))
            }
        }
    }
}

impl Debug for Block {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        self.header.fmt(f)
    }
}

impl SizedMsg for Block {
    fn size(&self) -> Result<usize, MailboxError> {
        Ok(self.header.size()? + self.txns.size()?)
    }
}
