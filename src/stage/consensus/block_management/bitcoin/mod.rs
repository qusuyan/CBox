mod basic;
use basic::BitcoinBlockManagement;

mod eager;
use eager::BitcoinEagerBlockManagement;

use super::{BlockManagement, CurBlockState};
use crate::config::BitcoinConfig;
use crate::peers::PeerMessenger;
use crate::vcores::VCoreGroup;
use crate::NodeId;

use atomic_float::AtomicF64;
use std::sync::Arc;

// sha256 in rust takes ~ 1.9 us for a 80 byte block header
const HEADER_HASH_TIME: f64 = 0.0000019;

pub fn new(
    id: NodeId,
    config: BitcoinConfig,
    delay: Arc<AtomicF64>,
    core_group: Arc<VCoreGroup>,
    peer_messenger: Arc<PeerMessenger>,
) -> Box<dyn BlockManagement> {
    match config {
        BitcoinConfig::Basic { config } => Box::new(BitcoinBlockManagement::new(
            id,
            config,
            delay,
            core_group,
            peer_messenger,
        )),
        BitcoinConfig::Eager { config } => Box::new(BitcoinEagerBlockManagement::new(
            id,
            config,
            delay,
            core_group,
            peer_messenger,
        )),
    }
}
