use get_size::GetSize;
use serde::{Deserialize, Serialize};

use crate::{
    protocol::crypto::{Hash, PubKey, Signature},
    CopycatError, CryptoScheme,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AvalancheTxn {
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
    Noop {
        parents: Vec<Hash>,
    },
    PlaceHolder, // place holders to replace invalid txns in a batch / block
}

impl AvalancheTxn {
    pub async fn validate(&self, crypto: CryptoScheme) -> Result<bool, CopycatError> {
        match self {
            AvalancheTxn::Send {
                sender,
                in_utxo,
                sender_signature,
                ..
            } => {
                let serialized_in_txo = bincode::serialize(in_utxo)?;
                crypto
                    .verify(sender, &serialized_in_txo, sender_signature)
                    .await
            }
            AvalancheTxn::Grant { .. } => Ok(true),
            AvalancheTxn::Noop { .. } | AvalancheTxn::PlaceHolder => {
                unreachable!("Validating Noop / PlaceHolder txns")
            }
        }
    }
}

impl GetSize for AvalancheTxn {}
