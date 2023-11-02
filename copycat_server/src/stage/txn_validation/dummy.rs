use copycat_utils::NodeId;

use super::TxnValidation;

pub struct DummyTxnValidation {
    _id: NodeId,
}

impl DummyTxnValidation {
    pub fn new(id: NodeId) -> Self {
        Self { _id: id }
    }
}

impl<TxnType> TxnValidation<TxnType> for DummyTxnValidation {
    fn validate(&self, _txn: &TxnType) -> bool {
        true
    }
}
