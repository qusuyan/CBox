// use crate::chain::{ChainType, DummyBlock};
// use crate::stage::{
//     commit,
//     consensus::{block_creation, block_dissemination, block_validation, decide},
//     pacemaker, txn_dissemination, txn_validation,
// };

// pub enum SystemCompose<TxnType> {
//     Dummy {
//         txn_validation_stage: Box<txn_validation::DummyTxnValidation>,
//         txn_dissemination_stage:
//             Box<txn_dissemination::BroadcastTxnDissemination<TxnType, DummyBlock<TxnType>>>,
//         pacemaker_stage: Box<pacemaker::DummyPacemaker>,
//         block_creation_stage: block_creation::DummyBlockCreation<TxnType>,
//         block_dissemination_stage:
//             block_dissemination::BroadcastBlockDissemination<TxnType, DummyBlock<TxnType>>,
//         block_validation_stage: block_validation::DummyBlockValidation,
//         decision_stage: decide::DummyDecision,
//         commit_type: commit::DummyCommit<TxnType>,
//     },
// }

// fn get_system_compose<TxnType>(chain: ChainType) -> SystemCompose<TxnType> {
//     match chain {
//         ChainType::Dummy => SystemCompose::Dummy {
//             txn_validation_stage: Box::new(txn_validation::DummyTxnValidation::new()),
//             txn_dissemination_stage: (),
//             pacemaker_stage: (),
//             block_creation_stage: (),
//             block_dissemination_stage: (),
//             block_validation_stage: (),
//             decision_stage: (),
//             commit_type: (),
//         },
//     }
// }

// pub trait SystemCompose {
//     type TxnType;
//     type BlockType;

//     fn get_txn_validation_stage() -> Box<dyn TxnValidation<Self::TxnType>>;
//     fn txn_dissemination_stage() -> Box<dyn TxnDissemination<Self::TxnType>>;
//     fn pacemaker_stage() -> Box<dyn Pacemaker>;
//     fn block_creation_stage() -> Box<dyn BlockCreation<Self::TxnType, Self::BlockType>>;
//     fn block_dissemination_stage() -> Box<dyn BlockDissemination<Self::BlockType>>;
//     fn block_validation_stage() -> Box<dyn BlockValidation<Self::BlockType>>;
//     fn decision_stage() -> Box<dyn Decision<Self::BlockType>>;
//     fn commit_type() -> Box<dyn Commit<Self::BlockType>>;
// }

// pub fn get_chain_compose(chain: ChainType) -> Box<dyn SystemCompose> {
//     match chain {
//         ChainType::Dummy => SystemCompose {
//             txn_validation_stage: TxnValidationType::Dummy,
//             txn_dissemination_stage: TxnDisseminationType::Broadcast,
//             pacemaker_stage: PacemakerType::Dummy,
//             block_creation_stage: BlockCreationType::Dummy,
//             block_dissemination_stage: BlockDisseminationType::Broadcast,
//             block_validation_stage: BlockValidationType::Dummy,
//             decision_stage: DecisionType::Dummy,
//             commit_type: CommitType::Dummy,
//         },
//         ChainType::Bitcoin => {
//             todo!();
//         }
//     }
// }
