mod basic;
use basic::BitcoinBlockManagement;

mod proposer;
use proposer::BitcoinProposerBlockManagement;

mod builder;
use builder::BitcoinBuilderBlockManagement;

use super::{BlockManagement, CurBlockState};
use crate::config::BitcoinConfig;
use crate::peers::PeerMessenger;
use crate::vcores::VCoreGroup;
use crate::NodeId;

use std::sync::Arc;

// sha256 in rust takes ~ 1.9 us for a 80 byte block header
const HEADER_HASH_TIME: f64 = 0.0000019;

pub fn new(
    id: NodeId,
    config: BitcoinConfig,
    core_group: Arc<VCoreGroup>,
    peer_messenger: Arc<PeerMessenger>,
) -> Box<dyn BlockManagement> {
    match config {
        BitcoinConfig::Basic { config } => Box::new(BitcoinBlockManagement::new(
            id,
            config,
            core_group,
            peer_messenger,
        )),
        BitcoinConfig::Proposer { config } => Box::new(BitcoinProposerBlockManagement::new(
            id,
            config,
            core_group,
            peer_messenger,
        )),
        BitcoinConfig::Builder { config } => Box::new(BitcoinBuilderBlockManagement::new(
            id,
            config,
            core_group,
            peer_messenger,
        )),
    }
}
