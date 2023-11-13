pub mod block;
pub mod transaction;

use block::Block;
use get_size::GetSize;
use serde::{Deserialize, Serialize};
use transaction::Txn;

#[derive(Copy, Clone, Debug, clap::ValueEnum)]
pub enum ChainType {
    Dummy,
    Bitcoin,
}

#[derive(Clone, Debug, Serialize, Deserialize, GetSize)]
pub enum MsgType {
    NewTxn { txn: Txn },
    NewBlock { blk: Block },
    ConsensusMsg { msg: Vec<u8> },
    PMakerMsg { msg: Vec<u8> },
}
