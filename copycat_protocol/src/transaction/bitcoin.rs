use get_size::GetSize;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, GetSize)]
pub struct BitCoinTxn {
    pub in_utxo: Vec<Vec<u8>>, // hashes of input utxos, owned by the same sender
    pub out_utxo: Vec<(u64, Vec<u8>)>, // how money is split and who the receivers are
    pub sender_signature: Vec<u8>, // signature of sender
}

// since transactions are created and never modified
unsafe impl Sync for BitCoinTxn {}
unsafe impl Send for BitCoinTxn {}
