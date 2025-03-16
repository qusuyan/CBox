mod basic;
use basic::DiemTxnValidation;

use super::TxnValidation;
use crate::config::DiemConfig;
use crate::NodeId;

pub fn new(id: NodeId, config: DiemConfig) -> Box<dyn TxnValidation> {
    match config {
        DiemConfig::Basic { config } => Box::new(DiemTxnValidation::new(id, config)),
    }
}
