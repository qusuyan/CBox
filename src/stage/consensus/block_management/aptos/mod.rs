mod basic;
use basic::AptosBlockManagement;

use super::{BlockManagement, CurBlockState};
use crate::config::AptosConfig;
use crate::peers::PeerMessenger;
use crate::protocol::crypto::signature::P2PSignature;
use crate::stage::DelayPool;
use crate::NodeId;

use std::sync::Arc;

pub fn new(
    id: NodeId,
    p2p_signature: P2PSignature,
    config: AptosConfig,
    delay: Arc<DelayPool>,
    peer_messenger: Arc<PeerMessenger>,
) -> Box<dyn BlockManagement> {
    match config {
        AptosConfig::Basic { config } => Box::new(AptosBlockManagement::new(
            id,
            p2p_signature,
            config,
            delay,
            peer_messenger,
        )),
    }
}
