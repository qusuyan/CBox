use copycat_utils::NodeId;

use super::TxnValidation;


pub struct DummyTxnValidation {
    id: NodeId, 
}

impl DummyTxnValidation {
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
        }
    }
}

impl<Txn> TxnValidation<Txn> for DummyTxnValidation {
    fn validate(&self, txn: &Txn) -> bool {
        true
    }
}