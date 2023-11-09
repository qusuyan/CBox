mod dummy;
pub use dummy::DummyBlock;

use get_size::GetSize;
use serde::{Deserialize, Serialize};

use std::sync::Arc;

pub trait BlockTrait<TxnType>: Sync + Send {
    fn fetch_txns(&self) -> Vec<Arc<TxnType>>;
}

// TODO: for better accuracy, we should implement GetSize manually so that message size
// matches the size after marshalling.
#[derive(Clone, Debug, Serialize, Deserialize, GetSize)]
pub enum Block<TxnType> {
    Dummy { blk: DummyBlock<TxnType> },
    Bitcoin,
}

impl<TxnType> BlockTrait<TxnType> for Block<TxnType>
where
    TxnType: Sync + Send,
{
    fn fetch_txns(&self) -> Vec<Arc<TxnType>> {
        match self {
            Block::Dummy { blk } => blk.fetch_txns(),
            _ => todo!(),
        }
    }
}
