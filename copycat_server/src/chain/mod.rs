use crate::composition::SystemCompose;
use crate::stage::{
    commit::CommitType, consensus::block_creation::BlockCreationType,
    consensus::block_dissemination::BlockDisseminationType,
    consensus::block_validation::BlockValidationType, consensus::decide::DecisionType,
    pacemaker::PacemakerType, txn_dissemination::TxnDisseminationType,
    txn_validation::TxnValidationType,
};

use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Debug, clap::ValueEnum)]
pub enum ChainType {
    Bitcoin,
    Dummy,
}

pub fn get_chain_compose(chain: ChainType) -> SystemCompose {
    match chain {
        ChainType::Dummy => SystemCompose {
            txn_validation_stage: TxnValidationType::Dummy,
            txn_dissemination_stage: TxnDisseminationType::Broadcast,
            pacemaker_stage: PacemakerType::Dummy,
            block_creation_stage: BlockCreationType::Dummy,
            block_dissemination_stage: BlockDisseminationType::Broadcast,
            block_validation_stage: BlockValidationType::Dummy,
            decision_stage: DecisionType::Dummy,
            commit_type: CommitType::Dummy,
        },
        ChainType::Bitcoin => {
            todo!();
        }
    }
}

pub trait BlockTrait<TxnType>: Sync + Send {
    fn fetch_txns(&self) -> Vec<Arc<TxnType>>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DummyBlock<TxnType> {
    txns: Vec<Arc<TxnType>>,
}

impl<TxnType> DummyBlock<TxnType> {
    pub fn new(txns: Vec<Arc<TxnType>>) -> Self {
        Self { txns }
    }
}

impl<TxnType> BlockTrait<TxnType> for DummyBlock<TxnType>
where
    TxnType: Sync + Send,
{
    fn fetch_txns(&self) -> Vec<Arc<TxnType>> {
        self.txns.clone()
    }
}
