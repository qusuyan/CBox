pub mod block;
pub mod crypto;
pub mod transaction;
pub mod types;

pub use crypto::signature::SignatureScheme;
pub use crypto::threshold_signature::ThresholdSignatureScheme;
use mailbox_client::{MailboxError, SizedMsg};

use crate::NodeId;
use block::Block;
use transaction::Txn;

use std::sync::Arc;

use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, Serialize, Deserialize, clap::ValueEnum)]
pub enum ChainType {
    Dummy,
    Bitcoin,
    Avalanche,
    ChainReplication,
    Diem,
    Aptos,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DissemPattern {
    Broadcast,
    Gossip,
    Sample { sample_size: usize },
    Linear { order: Vec<NodeId> },
    Passthrough,
    Narwhal,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MsgType {
    NewTxn { txn_batch: Vec<Arc<Txn>> },
    NewBlock { blk: Arc<Block> },
    BlkDissemMsg { msg: Vec<u8> },
    ConsensusMsg { msg: Vec<u8> },
    PMakerMsg { msg: Vec<u8> },
    BlockReq { msg: Vec<u8> },
}

impl SizedMsg for MsgType {
    fn size(&self) -> Result<usize, MailboxError> {
        match self {
            MsgType::NewTxn { txn_batch } => txn_batch.size(),
            MsgType::NewBlock { blk } => blk.size(),
            MsgType::BlkDissemMsg { msg } => Ok(msg.len()),
            MsgType::ConsensusMsg { msg } => Ok(msg.len()),
            MsgType::PMakerMsg { msg } => Ok(msg.len()),
            MsgType::BlockReq { msg } => Ok(msg.len()),
        }
    }
}
