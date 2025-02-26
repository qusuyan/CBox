use get_size::GetSize;
use serde::{Deserialize, Serialize};

use crate::{
    protocol::crypto::{sha256, Hash, PubKey, Signature},
    CopycatError, SignatureScheme,
};

use tokio::time::Duration;

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
        script_bytes: u64,           // size of the script
        script_runtime: Duration,    // how long does it take to run the script
        script_succeed: bool,        // if the script should fail
    },
    // used only for setup
    Grant {
        out_utxo: u64,
        receiver: PubKey,
    },
}

impl BitcoinTxn {
    pub fn compute_id(&self) -> Result<Hash, CopycatError> {
        let serialized = bincode::serialize(self)?;
        let id = sha256(&serialized)?;
        Ok(id)
    }

    pub fn validate(&self, crypto: SignatureScheme) -> Result<(bool, f64), CopycatError> {
        match self {
            BitcoinTxn::Send {
                sender,
                in_utxo,
                sender_signature,
                ..
            } => {
                let serialized_in_txo = bincode::serialize(in_utxo)?;
                crypto.verify(sender, &serialized_in_txo, sender_signature)
            }
            BitcoinTxn::Grant { .. } | BitcoinTxn::Incentive { .. } => Ok((true, 0f64)),
        }
    }

    pub fn get_size(&self) -> usize {
        let mut size = GetSize::get_size(self);
        if let BitcoinTxn::Send { script_bytes, .. } = self {
            size += *script_bytes as usize;
        }
        size
    }
}

impl GetSize for BitcoinTxn {}
