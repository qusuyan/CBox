use crate::protocol::crypto::{sha256, Hash, PubKey, Signature};
use crate::{CopycatError, SignatureScheme};

use std::time::Duration;

use get_size::GetSize;
use serde::{Deserialize, Serialize};

// https://github.com/diem/diem/blob/latest/types/src/transaction/mod.rs
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DiemTxn {
    Txn {
        sender: u64, // address
        seqno: u64,
        payload: DiemPayload,
        max_gas_amount: u64,
        // following fields exists in Diem's RawTransaction structure but are not used during emulation
        gas_unit_price: u64,
        gas_currency_code: String,
        expiration_timestamp_secs: u64,
        chain_id: u8,
        // authenticator
        sender_key: PubKey,
        signature: Signature,
    },
    // used only for setup
    Grant {
        receiver: u64, // address
        receiver_key: PubKey,
        amount: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiemPayload {
    script_bytes: usize,
    script_runtime: Duration,
    script_succeed: bool,
    distinct_writes: u64,
}

impl DiemTxn {
    pub fn compute_id(&self) -> Result<Hash, CopycatError> {
        let serialized = bincode::serialize(self)?;
        let id = sha256(&serialized)?;
        Ok(id)
    }

    pub fn validate(&self, crypto: SignatureScheme) -> Result<(bool, f64), CopycatError> {
        match self {
            DiemTxn::Txn {
                sender,
                seqno,
                payload,
                max_gas_amount,
                gas_unit_price,
                gas_currency_code,
                expiration_timestamp_secs,
                chain_id,
                sender_key,
                signature,
            } => {
                let serialized = bincode::serialize(&(
                    sender,
                    seqno,
                    payload,
                    max_gas_amount,
                    gas_unit_price,
                    gas_currency_code,
                    expiration_timestamp_secs,
                    chain_id,
                ))?;
                crypto.verify(sender_key, &serialized, signature)
            }
            DiemTxn::Grant { .. } => Ok((true, 0f64)),
        }
    }

    pub fn get_size(&self) -> usize {
        let mut size = GetSize::get_size(&self);
        if let DiemTxn::Txn { payload, .. } = self {
            size -= GetSize::get_size(&payload);
            size += payload.script_bytes;
        }
        size
    }
}
