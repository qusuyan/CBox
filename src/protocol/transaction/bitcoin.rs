use get_size::GetSize;
use serde::{Deserialize, Serialize};

use crate::protocol::crypto::{Hash, PubKey, Signature};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BitcoinTxn {
    Incentive {
        out_utxo: u64,
        receiver: PubKey,
    },
    Send {
        sender: PubKey,              // who the sender is
        in_utxo: Vec<Hash>,          // hashes of input utxos, owned by the same sender
        receiver: PubKey,            // who the receiver is
        out_utxo: u64,               // how much money will be sent to the receiver
        remainder: u64,              // how much money left
        sender_signature: Signature, // signature of sender
    },
    // used only for setup
    Grant {
        out_utxo: u64,
        receiver: PubKey,
    },
}

impl GetSize for BitcoinTxn {}
