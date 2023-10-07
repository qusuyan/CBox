mod dummy;

use copycat_utils::NodeId;

use self::dummy::DummyTxnValidation;

use std::sync::Arc;

pub trait TxnValidation<Txn>: Send + Sync {
    fn validate(&self, txn: &Txn) -> bool;
}

pub enum TxnValidationType {
    DUMMY,
}

pub fn get_txn_validation<TxnType>(
    id: NodeId,
    txn_validation_type: TxnValidationType,
) -> Arc<dyn TxnValidation<TxnType>> {
    match txn_validation_type {
        TxnValidationType::DUMMY => Arc::new(DummyTxnValidation::new(id)),
    }
}
