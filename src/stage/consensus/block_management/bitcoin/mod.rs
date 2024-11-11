mod basic;
use basic::BitcoinBlockManagement;

mod proposer;

mod builder;

use super::{BlockManagement, CurBlockState};
use crate::config::BitcoinConfig;
use crate::peers::PeerMessenger;
use crate::vcores::VCoreGroup;
use crate::NodeId;

use std::sync::Arc;

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
        BitcoinConfig::Proposer { config } => todo!(),
        BitcoinConfig::Builder { config } => todo!(),
    }
}
