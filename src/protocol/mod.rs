pub mod block;
pub mod crypto;

pub use crypto::signature::SignatureScheme;
pub mod transaction;

use block::Block;
use get_size::GetSize;
use serde::{Deserialize, Serialize};
use transaction::Txn;

use std::sync::Arc;

use crate::NodeId;

#[derive(Copy, Clone, Debug, Serialize, Deserialize, clap::ValueEnum)]
pub enum ChainType {
    Dummy,
    Bitcoin,
    Avalanche,
    ChainReplication,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DissemPattern {
    Broadcast,
    Gossip,
    Sample { sample_size: usize },
    Linear { order: Vec<NodeId> },
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
