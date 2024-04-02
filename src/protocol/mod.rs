pub mod block;
pub mod crypto;
pub use crypto::CryptoScheme;
pub mod transaction;

use block::Block;
use crypto::Hash;
use serde::{Deserialize, Serialize};
use transaction::Txn;

#[derive(Copy, Clone, Debug, clap::ValueEnum)]
pub enum ChainType {
    Dummy,
    Bitcoin,
    Avalanche,
}

#[derive(Copy, Clone, Debug, clap::ValueEnum)]
pub enum DissemPattern {
    Broadcast,
    Gossip,
    Sample,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MsgType {
    NewTxn { txn_batch: Vec<Txn> },
    NewBlock { blk: Block },
    ConsensusMsg { msg: Vec<u8> },
    PMakerMsg { msg: Vec<u8> },
    BlockReq { blk_id: Hash },
    // BlockResp { id: Hash, blk: Block },
}
