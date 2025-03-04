mod dummy;
pub use dummy::DummyTxn;

mod bitcoin;
pub use bitcoin::BitcoinTxn;

mod avalanche;
pub use avalanche::AvalancheTxn;

mod diem;
pub use diem::{DiemPayload, DiemTxn};

use get_size::GetSize;
use serde::{Deserialize, Serialize};

use crate::protocol::crypto::Hash;
use crate::{CopycatError, SignatureScheme};

// TODO: for better accuracy, we should implement GetSize manually so that message size
// matches the size after marshalling.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Txn {
    Dummy { txn: DummyTxn },
    Bitcoin { txn: BitcoinTxn },
    Avalanche { txn: AvalancheTxn },
    Diem { txn: DiemTxn },
}

impl Txn {
    pub fn compute_id(&self) -> Result<Hash, CopycatError> {
        match self {
            Txn::Dummy { txn } => txn.compute_id(),
            Txn::Bitcoin { txn } => txn.compute_id(),
            Txn::Avalanche { txn } => txn.compute_id(),
            Txn::Diem { txn } => txn.compute_id(),
        }
    }

    pub fn validate(&self, crypto: SignatureScheme) -> Result<(bool, f64), CopycatError> {
        match self {
            Txn::Dummy { txn } => txn.validate(crypto),
            Txn::Bitcoin { txn } => txn.validate(crypto),
            Txn::Avalanche { txn } => txn.validate(crypto),
            Txn::Diem { txn } => txn.validate(crypto),
        }
    }
}

impl GetSize for Txn {
    fn get_size(&self) -> usize {
        match self {
            Txn::Dummy { txn } => txn.get_size(),
            Txn::Avalanche { txn } => txn.get_size(),
            Txn::Bitcoin { txn } => BitcoinTxn::get_size(&txn),
            Txn::Diem { txn } => DiemTxn::get_size(&txn),
        }
    }
}

// since transactions are created and never modified
unsafe impl Sync for Txn {}
unsafe impl Send for Txn {}
