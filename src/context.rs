use primitive_types::U256;

use crate::protocol::block::{Block, BlockHeader};
use crate::protocol::crypto::{sha256, Hash};
use crate::transaction::{AvalancheTxn, Txn};
use crate::CopycatError;

use std::fmt::{Debug, Formatter};
use std::sync::Arc;

#[derive(Debug)]
pub struct TxnCtx {
    pub id: Hash,
}

pub struct BlkCtx {
    pub id: Hash,
    pub txn_ctx: Vec<Arc<TxnCtx>>,
}

impl TxnCtx {
    pub fn from_txn(txn: &Txn) -> Result<Self, CopycatError> {
        let id = match txn {
            Txn::Dummy { txn } => txn.id.clone(),
            Txn::Bitcoin { .. } => {
                let serialized = bincode::serialize(txn)?;
                sha256(&serialized)?
            }
            Txn::Avalanche { txn: avax_txn } => match avax_txn {
                AvalancheTxn::PlaceHolder => U256::zero(),
                _ => {
                    let serialized = bincode::serialize(txn)?;
                    sha256(&serialized)?
                }
            },
        };

        Ok(TxnCtx { id })
    }
}

impl BlkCtx {
    fn get_blk_id(header: &BlockHeader) -> Result<Hash, CopycatError> {
        match header {
            BlockHeader::Dummy | BlockHeader::Avalanche { .. } => Ok(U256::zero()),
            BlockHeader::Bitcoin { .. } => {
                let serialized_header = bincode::serialize(header)?;
                Ok(sha256(&serialized_header)?)
            }
            BlockHeader::ChainReplication { blk_id } => Ok(*blk_id),
        }
    }

    pub fn from_blk(block: &Block) -> Result<Self, CopycatError> {
        let blk_id = Self::get_blk_id(&block.header)?;

        let mut txn_ctx = vec![];
        for txn in block.txns.iter() {
            txn_ctx.push(Arc::new(TxnCtx::from_txn(txn)?));
        }

        Ok(BlkCtx {
            id: blk_id,
            txn_ctx,
        })
    }

    pub fn from_header_and_txns(
        header: &BlockHeader,
        txn_ctx: Vec<Arc<TxnCtx>>,
    ) -> Result<Self, CopycatError> {
        let blk_id = Self::get_blk_id(&header)?;

        Ok(BlkCtx {
            id: blk_id,
            txn_ctx,
        })
    }
}

impl Debug for BlkCtx {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.debug_struct("BlkCtx")
            .field("id", &self.id)
            .field("num_txns", &self.txn_ctx.len())
            .finish()
    }
}
