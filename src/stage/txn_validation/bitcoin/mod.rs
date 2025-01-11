mod basic;
use basic::BitcoinTxnValidation;

use super::TxnValidation;
use crate::config::BitcoinConfig;
use crate::NodeId;

pub fn new(id: NodeId, _config: BitcoinConfig) -> Box<dyn TxnValidation> {
    Box::new(BitcoinTxnValidation::new(id))
}
