pub mod block;
pub mod crypto;

pub use crypto::CryptoScheme;
pub mod transaction;

use block::Block;
use get_size::GetSize;
use serde::{Deserialize, Serialize};
use transaction::Txn;

use std::sync::Arc;

#[derive(Copy, Clone, Debug, clap::ValueEnum)]
pub enum ChainType {
    Dummy,
    Bitcoin,
    Avalanche,
    ChainReplication,
}

#[derive(Copy, Clone, Debug, clap::ValueEnum)]
pub enum DissemPattern {
    Broadcast,
    Gossip,
    Sample,
    Linear,
    Passthrough,
}

#[derive(Clone, Debug, Serialize, Deserialize, GetSize)]
pub enum MsgType {
    NewTxn { txn_batch: Vec<Arc<Txn>> },
    NewBlock { blk: Arc<Block> },
    ConsensusMsg { msg: Vec<u8> },
    PMakerMsg { msg: Vec<u8> },
    BlockReq { msg: Vec<u8> },
    // BlockResp { id: Hash, blk: Block },
}
