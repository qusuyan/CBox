mod bitcoin;
pub use bitcoin::BitcoinTxn;

mod dummy;
pub use dummy::DummyTxn;

mod avalanche;
pub use avalanche::AvalancheTxn;

use get_size::GetSize;
use serde::{Deserialize, Serialize};

// TODO: for better accuracy, we should implement GetSize manually so that message size
// matches the size after marshalling.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, GetSize)]
pub enum Txn {
    Dummy { txn: DummyTxn },
    Bitcoin { txn: BitcoinTxn },
    Avalanche { txn: AvalancheTxn },
}

// since transactions are created and never modified
unsafe impl Sync for Txn {}
unsafe impl Send for Txn {}
