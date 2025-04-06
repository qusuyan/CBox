mod dummy;
pub use dummy::DummyTxn;

mod bitcoin;
pub use bitcoin::BitcoinTxn;

mod avalanche;
pub use avalanche::AvalancheTxn;

mod diem;
pub use diem::{DiemAccountAddress, DiemPayload, DiemTxn};

mod aptos;
pub use aptos::{get_aptos_addr, AptosAccountAddress, AptosPayload, AptosTxn};

use mailbox_client::{MailboxError, SizedMsg};
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
    Aptos { txn: AptosTxn },
}

impl Txn {
    pub fn compute_id(&self) -> Result<Hash, CopycatError> {
        match self {
            Txn::Dummy { txn } => txn.compute_id(),
            Txn::Bitcoin { txn } => txn.compute_id(),
            Txn::Avalanche { txn } => txn.compute_id(),
            Txn::Diem { txn } => txn.compute_id(),
            Txn::Aptos { txn } => txn.compute_id(),
        }
    }

    pub fn validate(&self, crypto: SignatureScheme) -> Result<(bool, f64), CopycatError> {
        match self {
            Txn::Dummy { txn } => txn.validate(crypto),
            Txn::Bitcoin { txn } => txn.validate(crypto),
            Txn::Avalanche { txn } => txn.validate(crypto),
            Txn::Diem { txn } => txn.validate(crypto),
            Txn::Aptos { txn } => txn.validate(crypto),
        }
    }
}

// since transactions are created and never modified
unsafe impl Sync for Txn {}
unsafe impl Send for Txn {}

impl SizedMsg for Txn {
    fn size(&self) -> Result<usize, MailboxError> {
        match self {
            Txn::Dummy { txn } => txn.size(),
            Txn::Bitcoin { txn } => txn.size(),
            Txn::Avalanche { txn } => txn.size(),
            Txn::Diem { txn } => txn.size(),
            Txn::Aptos { txn } => txn.size(),
        }
    }
}
