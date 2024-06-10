mod bitcoin;
pub use bitcoin::BitcoinTxn;

mod dummy;
pub use dummy::DummyTxn;

mod avalanche;
pub use avalanche::AvalancheTxn;

use get_size::GetSize;
use serde::{Deserialize, Serialize};

use crate::{CopycatError, CryptoScheme};

// TODO: for better accuracy, we should implement GetSize manually so that message size
// matches the size after marshalling.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Txn {
    Dummy { txn: DummyTxn },
    Bitcoin { txn: BitcoinTxn },
    Avalanche { txn: AvalancheTxn },
}

impl Txn {
    pub fn validate(&self, crypto: CryptoScheme) -> Result<(bool, f64), CopycatError> {
        match self {
            Txn::Dummy { .. } => Ok((true, 0f64)),
            Txn::Bitcoin { txn } => txn.validate(crypto),
            Txn::Avalanche { txn } => txn.validate(crypto),
        }
    }
}

impl GetSize for Txn {
    fn get_size(&self) -> usize {
        match self {
            Txn::Dummy { txn } => txn.get_size(),
            Txn::Avalanche { txn } => txn.get_size(),
            Txn::Bitcoin { txn } => BitcoinTxn::get_size(&txn),
        }
    }
}

// since transactions are created and never modified
unsafe impl Sync for Txn {}
unsafe impl Send for Txn {}
