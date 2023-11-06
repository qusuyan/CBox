use crate::stage::{
    commit::CommitType,
    consensus::{
        block_creation::BlockCreationType, block_dissemination::BlockDisseminationType,
        block_validation::BlockValidationType, decide::DecisionType,
    },
    pacemaker::PacemakerType,
    txn_dissemination::TxnDisseminationType,
    txn_validation::TxnValidationType,
};

pub struct SystemCompose {
    pub txn_validation_stage: TxnValidationType,
    pub txn_dissemination_stage: TxnDisseminationType,
    pub pacemaker_stage: PacemakerType,
    pub block_creation_stage: BlockCreationType,
    pub block_dissemination_stage: BlockDisseminationType,
    pub block_validation_stage: BlockValidationType,
    pub decision_stage: DecisionType,
    pub commit_type: CommitType,
}
