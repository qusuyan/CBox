mod basic;
use basic::BitcoinDecision;

use super::Decision;
use crate::config::BitcoinConfig;
use crate::NodeId;

pub fn new(id: NodeId, config: BitcoinConfig) -> Box<dyn Decision> {
    match config {
        BitcoinConfig::Basic { config } => Box::new(BitcoinDecision::new(id, config)),
    }
}
