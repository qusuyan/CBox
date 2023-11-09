pub mod block;

use get_size::GetSize;
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, clap::ValueEnum)]
pub enum ChainType {
    Dummy,
    Bitcoin,
}

#[derive(Clone, Debug, Serialize, Deserialize, GetSize)]
pub enum MsgType<TxnType, BlockType> {
    NewTxn { txn: TxnType },
    NewBlock { blk: BlockType },
    ConsensusMsg { msg: Vec<u8> },
    PMakerMsg { msg: Vec<u8> },
}
