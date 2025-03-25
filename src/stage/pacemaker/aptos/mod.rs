mod basic;
use basic::AptosPmaker;

use super::Pacemaker;
use crate::config::AptosConfig;
use crate::protocol::crypto::signature::P2PSignature;
use crate::NodeId;

pub fn new(id: NodeId, config: AptosConfig, p2p_signature: P2PSignature) -> Box<dyn Pacemaker> {
    match config {
        AptosConfig::Basic { config } => Box::new(AptosPmaker::new(id, config, p2p_signature)),
    }
}
