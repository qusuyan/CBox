use get_size::GetSize;
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

// TODO: use len() instead of get_size() for signatures
impl GetSize for BlockHeader {
    fn get_size(&self) -> usize {
        match self {
            BlockHeader::Dummy => 0,
            BlockHeader::Bitcoin { .. } => 76,
            BlockHeader::Avalanche { signature, .. } => 56 + signature.len(),
            BlockHeader::ChainReplication { .. } => 32,
            BlockHeader::Diem {
                block,
                last_round_tc,
                high_commit_qc,
                signature,
            } => {
                let tc_size = match last_round_tc {
                    Some(tc) => 1 + tc.get_size(),
                    None => 1,
                };
                block.get_size() + tc_size + high_commit_qc.get_size() + signature.len()
            }
            BlockHeader::Aptos {
                certificates,
                signature,
                ..
            } => 48 + certificates.get_size() + signature.len(),
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

impl GetSize for Block {
    fn get_size(&self) -> usize {
        self.header.get_size()
            + self
                .txns
                .iter()
                .map(|txn| txn.as_ref().get_size())
                .sum::<usize>()
    }
}

impl Debug for Block {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        self.header.fmt(f)
    }
}

#[cfg(test)]
mod blk_header_get_size_test {
    use super::BlockHeader;
    use crate::protocol::crypto::Hash;
    use crate::protocol::types::aptos::CoA;
    use crate::protocol::types::diem::DiemBlock;
    use crate::protocol::types::diem::GENESIS_QC;

    use get_size::GetSize;
    use primitive_types::U256;

    #[test]
    fn blk_header_get_size() {
        let dummy_header = BlockHeader::Dummy;
        println!("{}", dummy_header.get_size());

        let cr_header = BlockHeader::ChainReplication {
            blk_id: Hash(U256::zero()),
        };
        println!("{}", cr_header.get_size());

        let avax_header = BlockHeader::Avalanche {
            proposer: 0,
            id: 0,
            merkle_root: Hash(U256::zero()),
            depth: 0,
            signature: vec![0u8; 64],
        };
        println!("{}", avax_header.get_size());

        let diem_block = DiemBlock {
            proposer: 0,
            round: 0,
            state_id: Hash(U256::zero()),
            qc: GENESIS_QC.clone(),
        };
        let diem_header = BlockHeader::Diem {
            block: diem_block,
            last_round_tc: None,
            high_commit_qc: GENESIS_QC.clone(),
            signature: vec![0u8; 64],
        };
        println!("{}", diem_header.get_size());

        let coa = CoA {
            sender: 0,
            round: 0,
            digest: Hash(U256::zero()),
            signature: vec![0u8; 64],
        };
        let aptos_header = BlockHeader::Aptos {
            sender: 0,
            round: 0,
            certificates: vec![coa; 3],
            merkle_root: Hash(U256::zero()),
            signature: vec![0u8; 64],
        };
        println!("{}", aptos_header.get_size());
    }
}
