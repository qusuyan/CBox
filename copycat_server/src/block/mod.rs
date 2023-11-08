mod dummy;
pub use dummy::DummyBlock;
use serde::{Deserialize, Serialize};

use std::sync::Arc;

#[derive(Copy, Clone, Debug, clap::ValueEnum)]
pub enum ChainType {
    Dummy,
    Bitcoin,
}

pub trait BlockTrait<TxnType>: Sync + Send {
    fn fetch_txns(&self) -> Vec<Arc<TxnType>>;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
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
