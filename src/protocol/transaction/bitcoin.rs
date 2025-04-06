use mailbox_client::{MailboxError, SizedMsg};
use serde::{Deserialize, Serialize};

use crate::{
    protocol::crypto::{sha256, Hash, PubKey, Signature},
    CopycatError, SignatureScheme,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
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
        script_bytes: usize,         // size of the script
        script_runtime_sec: f64,     // how long does it take to run the script
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
        let id = sha256(&self)?;
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
}

impl SizedMsg for BitcoinTxn {
    fn size(&self) -> Result<usize, MailboxError> {
        let size = match self {
            BitcoinTxn::Incentive { receiver, .. } => {
                8 + receiver.len() // out_utxo + receiver
            }
            BitcoinTxn::Send {
                sender,
                in_utxo,
                receiver,
                script_bytes,
                sender_signature,
                ..
            } => {
                sender.len()                // sender pubkey
                + 8 + 32 * in_utxo.len()    // in_utxo: length + content
                + receiver.len()            // receiver pubkey
                + 8 + 8                     // out_utxo + remainder
                + sender_signature.len()    // sender_signature
                + *script_bytes // script
            }
            BitcoinTxn::Grant { receiver, .. } => {
                8 + receiver.len() // out_utxo + receiver
            }
        };
        Ok(size)
    }
}
