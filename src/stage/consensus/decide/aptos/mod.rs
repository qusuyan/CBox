mod diem;
use diem::AptosDiemDecision;

use super::Decision;
use crate::config::AptosConfig;
use crate::peers::PeerMessenger;
use crate::protocol::crypto::signature::P2PSignature;
use crate::protocol::crypto::threshold_signature::ThresholdSignature;
use crate::stage::DelayPool;
use crate::NodeId;

use std::sync::Arc;

pub fn new(
    id: NodeId,
    p2p_signature: P2PSignature,
    threshold_signature: Arc<dyn ThresholdSignature>,
    config: AptosConfig,
    peer_messenger: Arc<PeerMessenger>,
    delay: Arc<DelayPool>,
) -> Box<dyn Decision> {
    match config {
        AptosConfig::Basic { config } => Box::new(AptosDiemDecision::new(
            id,
            p2p_signature,
            threshold_signature,
            config,
            peer_messenger,
            delay,
        )),
    }
}
