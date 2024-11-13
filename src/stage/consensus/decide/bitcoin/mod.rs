mod basic;
use basic::BitcoinDecision;

mod proposer;
use proposer::BitcoinProposerDecision;

mod builder;
use builder::BitcoinBuilderDecision;

use super::Decision;
use crate::config::BitcoinConfig;
use crate::protocol::crypto::Hash;
use crate::NodeId;

struct EndorseMsg {
    blk_id: Hash,
    nonce: u32,
}

pub fn new(id: NodeId, config: BitcoinConfig) -> Box<dyn Decision> {
    match config {
        BitcoinConfig::Basic { config } => Box::new(BitcoinDecision::new(id, config)),
        BitcoinConfig::Proposer { config } => Box::new(BitcoinProposerDecision::new(id, config)),
        BitcoinConfig::Builder { config } => Box::new(BitcoinBuilderDecision::new(id, config)),
    }
}
