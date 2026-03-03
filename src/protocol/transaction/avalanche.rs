use mailbox_client::{MailboxError, SizedMsg};
use primitive_types::U256;
use serde::{Deserialize, Serialize};

use crate::{
    protocol::crypto::{sha256, Hash, PubKey, Signature},
    CopycatError, SignatureScheme,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AvalancheTxn {
    Send {
        sender: PubKey,              // who the sender is
        in_utxo: Vec<Hash>,          // hashes of input utxos, owned by the same sender
        receiver: PubKey,            // who the receiver is
        out_utxo: u64,               // how much money will be sent to the receiver
        remainder: u64,              // how much money left
        sender_signature: Signature, // signature of sender
        payload_size: usize,         // size of the payload
    },
    // used only for setup
    Grant {
        out_utxo: u64,
        receiver: PubKey,
        nonce: u64,
    },
    Noop {
        parents: Vec<Hash>,
    },
    PlaceHolder, // place holders to replace invalid txns in a batch / block
}

impl AvalancheTxn {
    pub fn compute_id(&self) -> Result<Hash, CopycatError> {
        let id = match self {
            AvalancheTxn::PlaceHolder => Hash(U256::zero()),
            _ => sha256(&self)?,
        };

        Ok(id)
    }

    pub fn validate(&self, crypto: SignatureScheme) -> Result<(bool, f64), CopycatError> {
        match self {
            AvalancheTxn::Send {
                sender,
                in_utxo,
                sender_signature,
                ..
            } => {
                let serialized_in_txo = bincode::serialize(in_utxo)?;
                crypto.verify(sender, &serialized_in_txo, sender_signature)
            }
            AvalancheTxn::Grant { .. } => Ok((true, 0f64)),
            AvalancheTxn::Noop { .. } | AvalancheTxn::PlaceHolder => {
                unreachable!("Validating Noop / PlaceHolder txns")
            }
        }
    }
}

impl SizedMsg for AvalancheTxn {
    fn size(&self) -> Result<usize, MailboxError> {
        let size = match self {
            AvalancheTxn::Send {
                in_utxo,
                payload_size,
                receiver,
                sender_signature,
                ..
            } => {
                32                          // sender pubkey
                 + 8 + 32 * in_utxo.len()   // in_utxo
                 + receiver.len() + 8 + 8   // receiver pubkey + out_utxo + remainder
                 + sender_signature.len()   // sender_signature 
                 + *payload_size // script
            }
            AvalancheTxn::Grant { receiver, .. } => {
                8 + receiver.len() + 8 // out_utxo + receiver + nonce
            }
            AvalancheTxn::Noop { parents } => parents.len() * 32,
            AvalancheTxn::PlaceHolder => 0,
        };
        Ok(size)
    }
}
