use super::TxnValidation;
use crate::state::{BitcoinState, ChainState};
use copycat_protocol::transaction::{BitcoinTxn, Txn};
use copycat_protocol::CryptoScheme;
use copycat_utils::CopycatError;

use async_trait::async_trait;

use std::sync::Arc;

pub struct BitcoinTxnValidation {
    state: Arc<BitcoinState>,
    crypto_scheme: CryptoScheme,
}

impl BitcoinTxnValidation {
    pub fn new(state: Arc<ChainState>, crypto_scheme: CryptoScheme) -> Self {
        match state.as_ref() {
            ChainState::Bitcoin { state } => Self {
                state: unsafe { Arc::from_raw(state) }, //FIXME
                crypto_scheme,
            },
            _ => unreachable!(),
        }
    }
}

#[async_trait]
impl TxnValidation for BitcoinTxnValidation {
    async fn validate(&self, txn: &Txn) -> Result<bool, CopycatError> {
        let bitcoin_txn = match txn {
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
        if !self.crypto_scheme.verify(
            txn_sender,
            &bincode::serialize(txn_in_utxo)?,
            txn_sender_signature,
        )? {
            return Ok(false);
        }

        let txns = self.state.txn_pool.read().await;
        let mut input_value = 0;
        for in_utxo_hash in txn_in_utxo.iter() {
            // first check that input transactions exists, we can check for double spend later as a block
            // add values together to find total input value
            let value = match txns.get(in_utxo_hash) {
                Some(txn) => match txn {
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
                },
                None => return Ok(false),
            };
            input_value += value;
        }

        // check if the input values match output values
        if input_value != txn_out_utxo + txn_remainder {
            return Ok(false);
        }

        Ok(true)
    }
}
