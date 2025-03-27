use crate::protocol::block::{Block, BlockHeader};
use crate::protocol::crypto::Hash;
use crate::transaction::Txn;
use crate::CopycatError;

use std::fmt::{Debug, Formatter};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TxnCtx {
    pub id: Hash,
}

#[derive(Eq, PartialEq)]
pub struct BlkCtx {
    pub id: Hash,
    pub invalid: bool,
    pub txn_ctx: Vec<Arc<TxnCtx>>,
}

impl TxnCtx {
    pub fn from_txn(txn: &Txn) -> Result<Self, CopycatError> {
        let id = txn.compute_id()?;
        Ok(TxnCtx { id })
    }
}

impl BlkCtx {
    pub fn from_blk(block: &Block) -> Result<Self, CopycatError> {
        let mut txn_ctx = vec![];
        for txn in block.txns.iter() {
            txn_ctx.push(Arc::new(TxnCtx::from_txn(txn)?));
        }

        let blk_id = Block::compute_id(&block.header, &txn_ctx)?;

        Ok(BlkCtx {
            id: blk_id,
            invalid: false,
            txn_ctx,
        })
    }

    pub fn from_header_and_txns(
        header: &BlockHeader,
        txn_ctx: Vec<Arc<TxnCtx>>,
    ) -> Result<Self, CopycatError> {
        let blk_id = Block::compute_id(header, &txn_ctx)?;

        Ok(BlkCtx {
            id: blk_id,
            invalid: false,
            txn_ctx,
        })
    }

    pub fn from_id_and_txns(id: Hash, txn_ctx: Vec<Arc<TxnCtx>>) -> Self {
        BlkCtx {
            id,
            invalid: false,
            txn_ctx,
        }
    }

    pub fn invalidate(&self) -> Self {
        Self {
            id: self.id,
            invalid: true,
            txn_ctx: self.txn_ctx.clone(),
        }
    }
}

impl Debug for BlkCtx {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.debug_struct("BlkCtx")
            .field("id", &self.id)
            .field("num_txns", &self.txn_ctx.len())
            .field("is_invalid", &self.invalid)
            .finish()
    }
}
