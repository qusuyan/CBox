use crate::protocol::crypto::{sha256, Hash, PubKey, Signature};
use crate::{CopycatError, SignatureScheme};

use get_size::GetSize;
use serde::{Deserialize, Serialize};

pub type DiemAccountAddress = [u8; 16];

// https://github.com/diem/diem/blob/latest/types/src/transaction/mod.rs
#[derive(Debug, Clone, Serialize, Deserialize, GetSize)]
pub enum DiemTxn {
    Txn {
        sender: DiemAccountAddress,
        seqno: u64,
        payload: DiemPayload,
        max_gas_amount: u64,
        // following fields exists in Diem's RawTransaction structure but are not used during emulation
        gas_unit_price: u64,
        // gas_currency_code: String,
        expiration_timestamp_secs: u64,
        chain_id: u8,
        // authenticator
        sender_key: PubKey,
        signature: Signature,
    },
    // used only for setup
    Grant {
        receiver: DiemAccountAddress,
        receiver_key: PubKey,
        amount: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, GetSize)]
pub struct DiemPayload {
    pub script_bytes: usize,
    pub script_runtime_sec: f64,
    pub script_succeed: bool,
    pub distinct_writes: usize,
}

impl DiemTxn {
    pub fn compute_id(&self) -> Result<Hash, CopycatError> {
        let id = sha256(&self)?;
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
                // gas_currency_code,
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
                    // gas_currency_code,
                    expiration_timestamp_secs,
                    chain_id,
                ))?;
                crypto.verify(sender_key, &serialized, signature)
            }
            DiemTxn::Grant { .. } => Ok((true, 0f64)),
        }
    }

    pub fn get_size(&self) -> usize {
        let mut size = GetSize::get_size(self);
        if let DiemTxn::Txn { payload, .. } = self {
            size -= GetSize::get_size(payload);
            size += payload.script_bytes;
        }
        size
    }
}

#[cfg(test)]
mod diem_txn_test {

    use super::{super::Txn, DiemPayload, DiemTxn};
    use crate::protocol::crypto::signature::SignatureScheme;
    use crate::CopycatError;

    use get_size::GetSize;
    use std::sync::Arc;

    #[test]
    fn test_txn_size_correct() -> Result<(), CopycatError> {
        let expected_size = 1024;
        let script_size = expected_size - 184;

        let scheme = SignatureScheme::DummyECDSA;
        let (pk, sk) = scheme.gen_key_pair(0);
        let data = (
            [0u8; 16],
            0u64,
            DiemPayload {
                script_bytes: script_size,
                script_runtime_sec: 0f64,
                script_succeed: true,
                distinct_writes: 3,
            },
            5,
            0,
            // "XUS".to_owned(),
            1611792876,
            4,
        );
        let serialized = bincode::serialize(&data)?;
        let (signature, _) = scheme.sign(&sk, &serialized)?;
        let (
            sender,
            seqno,
            payload,
            max_gas_amount,
            gas_unit_price,
            // gas_currency_code,
            expiration_timestamp_secs,
            chain_id,
        ) = data;

        let diem_txn = DiemTxn::Txn {
            sender, // address
            seqno,
            payload,
            max_gas_amount,
            gas_unit_price,
            // gas_currency_code,
            expiration_timestamp_secs,
            chain_id,
            sender_key: pk,
            signature,
        };

        let txn = Arc::new(Txn::Diem { txn: diem_txn });

        println!("{}", txn.get_size());
        assert!(txn.get_size() == expected_size);

        Ok(())
    }
}
