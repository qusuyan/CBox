use crate::protocol::crypto::{sha256, Hash, PubKey, Signature};
use crate::{CopycatError, SignatureScheme};

use mailbox_client::{MailboxError, SizedMsg};
use serde::{Deserialize, Serialize};

pub type AptosAccountAddress = [u8; 32];

pub fn get_aptos_addr(pk: &PubKey) -> Result<AptosAccountAddress, CopycatError> {
    let hash = sha256(pk)?;
    Ok(hash.0.into())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AptosTxn {
    Txn {
        sender: AptosAccountAddress,
        seqno: u64,
        payload: AptosPayload,
        max_gas_amount: u64,
        gas_unit_price: u64,
        expiration_timestamp_secs: u64,
        chain_id: u8,
        // authenticator
        sender_key: PubKey,
        signature: Signature,
    },
    // used only for setup
    Grant {
        receiver_key: PubKey,
        amount: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AptosPayload {
    pub script_bytes: usize,
    pub script_runtime_sec: f64,
    pub script_succeed: bool,
    pub distinct_writes: usize,
}

impl AptosTxn {
    pub fn compute_id(&self) -> Result<Hash, CopycatError> {
        let id = sha256(&self)?;
        Ok(id)
    }

    pub fn validate(&self, crypto: SignatureScheme) -> Result<(bool, f64), CopycatError> {
        match self {
            AptosTxn::Txn {
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
            AptosTxn::Grant { .. } => Ok((true, 0f64)),
        }
    }
}

impl SizedMsg for AptosTxn {
    fn size(&self) -> Result<usize, MailboxError> {
        let size = match self {
            AptosTxn::Txn {
                sender,
                payload,
                sender_key,
                signature,
                ..
            } => {
                sender.len()            // sender addr
                + 8                     // seqno
                + payload.script_bytes  // script
                + 8 + 8 + 8 + 1         // max_gas_amount + gas_unit_price + expiration_timestamp_secs + chain_id
                + sender_key.len()      // sender pubkey
                + signature.len() // signature
            }
            AptosTxn::Grant { .. } => 32 + 8, // receiver_key + amount
        };
        Ok(size)
    }
}

#[cfg(test)]
mod aptos_txn_test {

    use mailbox_client::SizedMsg;

    use super::{super::Txn, AptosPayload, AptosTxn};
    use crate::protocol::crypto::signature::SignatureScheme;
    use crate::CopycatError;

    use std::sync::Arc;

    #[test]
    fn test_txn_size_correct() -> Result<(), CopycatError> {
        let expected_size = 1024usize;
        let script_size = expected_size - 161;

        let scheme = SignatureScheme::DummyECDSA;
        let (pk, sk) = scheme.gen_key_pair(0);

        let sender = [0u8; 32];
        let seqno = 0u64;
        let payload = AptosPayload {
            script_bytes: script_size,
            script_runtime_sec: 0f64,
            script_succeed: true,
            distinct_writes: 3,
        };
        let max_gas_amount = 5;
        let gas_unit_price = 0;
        // "XUS".to_owned(),
        let expiration_timestamp_secs = 1611792876;
        let chain_id = 4;

        let data = (
            &sender,
            &seqno,
            &payload,
            &max_gas_amount,
            &gas_unit_price,
            &expiration_timestamp_secs,
            &chain_id,
        );
        let serialized = bincode::serialize(&data)?;
        let (signature, _) = scheme.sign(&sk, &serialized)?;

        let aptos_txn = AptosTxn::Txn {
            sender,
            seqno,
            payload,
            max_gas_amount,
            gas_unit_price,
            expiration_timestamp_secs,
            chain_id,
            sender_key: pk,
            signature,
        };

        let txn = Arc::new(Txn::Aptos { txn: aptos_txn });

        let txn_size = txn.size()?;
        println!("{}", txn_size);
        assert!(txn_size == expected_size);

        Ok(())
    }
}
