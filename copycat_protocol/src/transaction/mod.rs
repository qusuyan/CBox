mod bitcoin;
pub use bitcoin::BitCoinTxn;

mod dummy;
pub use dummy::DummyTxn;

use get_size::GetSize;
use serde::{Deserialize, Serialize};

// TODO: for better accuracy, we should implement GetSize manually so that message size
// matches the size after marshalling.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, GetSize)]
pub enum Txn {
    Dummy { txn: DummyTxn },
    Bitcoin { txn: BitCoinTxn },
}
