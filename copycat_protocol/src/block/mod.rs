mod dummy;
pub use dummy::DummyBlock;

use get_size::GetSize;
use serde::{Deserialize, Serialize};

use std::sync::Arc;

use crate::transaction::Txn;

pub trait BlockTrait: Sync + Send {
    fn fetch_txns(&self) -> Vec<Arc<Txn>>;
}

// TODO: for better accuracy, we should implement GetSize manually so that message size
// matches the size after marshalling.
#[derive(Clone, Debug, Serialize, Deserialize, GetSize)]
pub enum Block {
    Dummy { blk: DummyBlock },
    Bitcoin,
}

impl Block {
    pub fn fetch_txns(&self) -> Vec<Arc<Txn>> {
        match self {
            Block::Dummy { blk } => blk.fetch_txns(),
            _ => todo!(),
        }
    }
}
