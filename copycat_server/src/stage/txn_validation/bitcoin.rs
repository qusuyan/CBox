use super::TxnValidation;
use copycat_protocol::crypto::{sha256, Hash};
use copycat_protocol::transaction::{BitcoinTxn, Txn};
use copycat_protocol::CryptoScheme;
use copycat_utils::CopycatError;

use async_trait::async_trait;

use std::collections::HashMap;
use std::sync::Arc;

pub struct BitcoinTxnValidation {
    txns_pool: HashMap<Hash, Arc<Txn>>,
    crypto_scheme: CryptoScheme,
}

impl BitcoinTxnValidation {
    pub fn new(crypto_scheme: CryptoScheme) -> Self {
        Self {
            txns_pool: HashMap::new(),
            crypto_scheme,
        }
    }
}

#[async_trait]
impl TxnValidation for BitcoinTxnValidation {
    async fn validate(&mut self, txn: Arc<Txn>) -> Result<bool, CopycatError> {
        let bitcoin_txn = match txn.as_ref() {
            Txn::Bitcoin { txn } => txn,
            _ => unreachable!(),
        };

        let (txn_sender, txn_in_utxo, txn_sender_signature) = match bitcoin_txn {
            BitcoinTxn::Send {
                sender,
                in_utxo,
                sender_signature,
                ..
            } => (sender, in_utxo, sender_signature),
            BitcoinTxn::Grant { .. } => {
                let hash = sha256(&bincode::serialize(&txn)?)?;
                self.txns_pool.insert(hash, txn);
                return Ok(true);
            }
            BitcoinTxn::Incentive { .. } => return Ok(false),
        };

        let serialized_txn = bincode::serialize(&txn)?;
        let hash = sha256(&serialized_txn)?;

        if self.txns_pool.get(&hash) != None {
            // txn has already been seem, ignoring...
            return Ok(false);
        }

        // first check if the signature is valid
        let serialized_in_txo = bincode::serialize(txn_in_utxo)?;
        if !self
            .crypto_scheme
            .verify(txn_sender, &serialized_in_txo, txn_sender_signature)?
        {
            return Ok(false);
        }

        self.txns_pool.insert(hash.clone(), txn);

        Ok(true)
    }
}
