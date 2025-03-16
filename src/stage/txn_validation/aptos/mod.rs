mod basic;
use basic::AptosTxnValidation;

use super::TxnValidation;
use crate::config::AptosConfig;
use crate::NodeId;

pub fn new(id: NodeId, config: AptosConfig) -> Box<dyn TxnValidation> {
    match config {
        AptosConfig::Basic { .. } => Box::new(AptosTxnValidation::new(id)),
    }
}
