use super::TxnValidation;
use crate::context::TxnCtx;
use crate::protocol::crypto::{sha256, Hash};
use crate::protocol::transaction::{BitcoinTxn, Txn};
use crate::protocol::CryptoScheme;
use crate::utils::CopycatError;

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
    async fn validate(&mut self, txn: Arc<Txn>) -> Result<Option<Arc<TxnCtx>>, CopycatError> {
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
                return Ok(Some(Arc::new(TxnCtx { id: hash })));
            }
            BitcoinTxn::Incentive { .. } => return Ok(None),
        };

        let serialized_txn = bincode::serialize(&txn)?;
        let hash = sha256(&serialized_txn)?;

        if self.txns_pool.get(&hash) != None {
            // txn has already been seem, ignoring...
            return Ok(None);
        }

        // first check if the signature is valid
        let serialized_in_txo = bincode::serialize(txn_in_utxo)?;
        if !self
            .crypto_scheme
            .verify(txn_sender, &serialized_in_txo, txn_sender_signature)
            .await?
        {
            return Ok(None);
        }

        self.txns_pool.insert(hash.clone(), txn);

        Ok(Some(Arc::new(TxnCtx { id: hash })))
    }
}
