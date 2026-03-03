mod basic;
use basic::AvalancheBlockManagement;

mod vote_no;
use vote_no::AvalancheVoteNoBlockManagement;

use super::{BlockManagement, CurBlockState};
use crate::config::AvalancheConfig;
use crate::peers::PeerMessenger;
use crate::protocol::crypto::signature::P2PSignature;
use crate::stage::DelayPool;
use crate::NodeId;

use std::sync::Arc;

pub fn new(
    id: NodeId,
    p2p_signature: P2PSignature,
    config: AvalancheConfig,
    peer_messenger: Arc<PeerMessenger>,
    delay: Arc<DelayPool>,
) -> Box<dyn BlockManagement> {
    match config {
        AvalancheConfig::Basic { config }
        | AvalancheConfig::Blizzard { config }
        | AvalancheConfig::NoCache { config } => Box::new(AvalancheBlockManagement::new(
            id,
            p2p_signature,
            config,
            peer_messenger,
            delay,
        )),
        AvalancheConfig::VoteNo { config } => {
            Box::new(AvalancheVoteNoBlockManagement::new(id, config))
        }
    }
}
