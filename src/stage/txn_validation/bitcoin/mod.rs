mod basic;
use basic::BitcoinTxnValidation;

use super::TxnValidation;
use crate::config::BitcoinConfig;
use crate::NodeId;

pub fn new(id: NodeId, config: BitcoinConfig) -> Box<dyn TxnValidation> {
    match config {
        BitcoinConfig::Basic { config } => Box::new(BitcoinTxnValidation::new(id, config)),
    }
}
