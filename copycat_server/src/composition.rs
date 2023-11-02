use crate::stage::{txn_dissemination::TxnDisseminationType, txn_validation::TxnValidationType};

pub struct SystemCompose {
    pub txn_validation_stage: TxnValidationType,
    pub txn_dissemination_stage: TxnDisseminationType,
}
