mod basic;
use basic::AvalancheTxnValidation;

mod vote_no;
use vote_no::AvalancheVoteNoTxnValidation;

use super::TxnValidation;
use crate::config::AvalancheConfig;
use crate::NodeId;

pub fn new(id: NodeId, config: AvalancheConfig) -> Box<dyn TxnValidation> {
    match config {
        AvalancheConfig::Basic { config } => Box::new(AvalancheTxnValidation::new(id, config)),
        AvalancheConfig::Blizzard { config } => Box::new(AvalancheTxnValidation::new(id, config)),
        AvalancheConfig::VoteNo { config } => {
            Box::new(AvalancheVoteNoTxnValidation::new(id, config))
        }
    }
}
