use super::TxnValidation;
use crate::context::TxnCtx;
use crate::protocol::crypto::Hash;
use crate::protocol::transaction::{AvalancheTxn, Txn};
use crate::protocol::CryptoScheme;
use crate::utils::CopycatError;

use async_trait::async_trait;

use std::collections::HashMap;
use std::sync::Arc;

pub struct AvalancheTxnValidation {
    txns_pool: HashMap<Hash, Arc<Txn>>,
    crypto_scheme: CryptoScheme,
}

impl AvalancheTxnValidation {
    pub fn new(crypto_scheme: CryptoScheme) -> Self {
        Self {
            txns_pool: HashMap::new(),
            crypto_scheme,
        }
    }
}

#[async_trait]
impl TxnValidation for AvalancheTxnValidation {
    async fn validate(&mut self, txn: Arc<Txn>) -> Result<Option<Arc<TxnCtx>>, CopycatError> {
        let avax_txn = match txn.as_ref() {
            Txn::Avalanche { txn } => txn,
            _ => unreachable!(),
        };

        let (txn_sender, txn_in_utxo, txn_sender_signature) = match avax_txn {
            AvalancheTxn::Send {
                sender,
                in_utxo,
                sender_signature,
                ..
            } => (sender, in_utxo, sender_signature),
            AvalancheTxn::Grant { .. } => {
                let txn_ctx = TxnCtx::from_txn(&txn)?;
                return Ok(Some(Arc::new(txn_ctx)));
            }
            AvalancheTxn::Noop { .. } | AvalancheTxn::PlaceHolder => {
                unreachable!();
            }
        };

        let txn_ctx = TxnCtx::from_txn(&txn)?;
        let hash = txn_ctx.id;

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

        Ok(Some(Arc::new(txn_ctx)))
    }
}
