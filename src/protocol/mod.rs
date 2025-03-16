pub mod block;
pub mod crypto;
pub mod transaction;
pub mod types;

pub use crypto::signature::SignatureScheme;
pub use crypto::threshold_signature::ThresholdSignatureScheme;

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
    Diem,
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

#[derive(Clone, Debug, Serialize, Deserialize, GetSize)]
pub enum MsgType {
    NewTxn { txn_batch: Vec<Arc<Txn>> },
    NewBlock { blk: Arc<Block> },
    BlkDissemMsg { msg: Vec<u8> },
    ConsensusMsg { msg: Vec<u8> },
    PMakerMsg { msg: Vec<u8> },
    BlockReq { msg: Vec<u8> },
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
