mod basic;
use basic::DiemDecision;

use super::Decision;
use crate::config::DiemConfig;
use crate::peers::PeerMessenger;
use crate::protocol::crypto::signature::P2PSignature;
use crate::protocol::crypto::threshold_signature::ThresholdSignature;
use crate::stage::DelayPool;
use crate::NodeId;

use std::sync::Arc;
use tokio::sync::mpsc;

pub fn new(
    id: NodeId,
    p2p_signature: P2PSignature,
    threshold_signature: Arc<dyn ThresholdSignature>,
    config: DiemConfig,
    peer_messenger: Arc<PeerMessenger>,
    pmaker_feedback_send: mpsc::Sender<Vec<u8>>,
    delay: Arc<DelayPool>,
) -> Box<dyn Decision> {
    match config {
        DiemConfig::Basic { config } => Box::new(DiemDecision::new(
            id,
            p2p_signature,
            threshold_signature,
            config,
            peer_messenger,
            pmaker_feedback_send,
            delay,
        )),
    }
}
