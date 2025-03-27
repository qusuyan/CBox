pub mod block;
pub mod crypto;
pub mod transaction;
pub mod types;

pub use crypto::signature::SignatureScheme;
pub use crypto::threshold_signature::ThresholdSignatureScheme;

use crate::NodeId;
use block::Block;
use transaction::Txn;

use std::sync::Arc;

use get_size::GetSize;
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

impl GetSize for MsgType {
    fn get_size(&self) -> usize {
        match self {
            MsgType::NewTxn { txn_batch } => {
                8 + txn_batch
                    .iter()
                    .map(|txn| txn.as_ref().get_size())
                    .sum::<usize>()
            }
            MsgType::NewBlock { blk } => blk.as_ref().get_size(),
            MsgType::BlkDissemMsg { msg } => msg.len(),
            MsgType::ConsensusMsg { msg } => msg.len(),
            MsgType::PMakerMsg { msg } => msg.len(),
            MsgType::BlockReq { msg } => msg.len(),
        }
    }
}

#[cfg(test)]
mod msgtype_test {

    use super::MsgType;
    use crate::protocol::transaction::BitcoinTxn;
    use crate::protocol::transaction::Txn;
    use crate::CopycatError;

    use get_size::GetSize;
    use std::sync::Arc;

    #[test]
    fn test_msg_size_correct() -> Result<(), CopycatError> {
        let txn_size = 0x100000;
        let txn = Txn::Bitcoin {
            txn: BitcoinTxn::Send {
                sender: vec![],
                in_utxo: vec![],
                receiver: vec![],
                out_utxo: 10,
                remainder: 10,
                sender_signature: vec![],
                script_bytes: 0x100000,
                script_runtime_sec: 0f64,
                script_succeed: true,
            },
        };

        let msg = MsgType::NewTxn {
            txn_batch: vec![Arc::new(txn)],
        };

        assert!(msg.get_size() > txn_size);

        Ok(())
    }
}
