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

        let (txn_sender, txn_in_utxo, txn_out_utxo, txn_remainder, txn_sender_signature) =
            match bitcoin_txn {
                BitcoinTxn::Send {
                    sender,
                    in_utxo,
                    receiver: _,
                    out_utxo,
                    remainder,
                    sender_signature,
                } => (sender, in_utxo, out_utxo, remainder, sender_signature),
                BitcoinTxn::Incentive { .. } => return Ok(false),
            };

        // first check if the signature is valid
        let serialized = bincode::serialize(txn_in_utxo)?;
        if !self
            .crypto_scheme
            .verify(txn_sender, &serialized, txn_sender_signature)?
        {
            return Ok(false);
        }

        let hash = sha256(&serialized)?;

        if self.txns_pool.get(&hash) != None {
            // txn has already been seem, ignoring...
            return Ok(false);
        }

        let mut input_value = 0;
        for in_utxo_hash in txn_in_utxo.iter() {
            // first check that input transactions exists, we can check for double spend later as a block
            // add values together to find total input value
            let utxo = match self.txns_pool.get(in_utxo_hash) {
                Some(txn) => match txn.as_ref() {
                    Txn::Bitcoin { txn } => txn,
                    _ => unreachable!(),
                },
                None => return Ok(false),
            };

            let value = match utxo {
                BitcoinTxn::Incentive { out_utxo, receiver } => {
                    if receiver == txn_sender {
                        out_utxo
                    } else {
                        return Ok(false);
                    }
                }
                BitcoinTxn::Send {
                    sender,
                    in_utxo: _,
                    receiver,
                    out_utxo,
                    remainder,
                    sender_signature: _,
                } => {
                    if receiver == txn_sender {
                        out_utxo
                    } else if sender == txn_sender {
                        remainder
                    } else {
                        return Ok(false);
                    }
                }
            };
            input_value += value;
        }

        // check if the input values match output values
        if input_value != txn_out_utxo + txn_remainder {
            return Ok(false);
        }

        self.txns_pool.insert(hash.clone(), txn);

        Ok(true)
    }
}
