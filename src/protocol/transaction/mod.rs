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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, GetSize)]
pub enum Txn {
    Dummy { txn: DummyTxn },
    Bitcoin { txn: BitcoinTxn },
    Avalanche { txn: AvalancheTxn },
}

impl Txn {
    pub async fn validate(&self, crypto: CryptoScheme) -> Result<bool, CopycatError> {
        match self {
            Txn::Dummy { .. } => Ok(true),
            Txn::Bitcoin { txn } => txn.validate(crypto).await,
            Txn::Avalanche { txn } => txn.validate(crypto).await,
        }
    }
}

// since transactions are created and never modified
unsafe impl Sync for Txn {}
unsafe impl Send for Txn {}
