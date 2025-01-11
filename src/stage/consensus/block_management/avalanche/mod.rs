mod basic;
use basic::AvalancheBlockManagement;

mod vote_no;
use vote_no::AvalancheVoteNoBlockManagement;

use super::{BlockManagement, CurBlockState};
use crate::config::AvalancheConfig;
use crate::peers::PeerMessenger;
use crate::NodeId;

use std::sync::Arc;

pub fn new(
    id: NodeId,
    config: AvalancheConfig,
    peer_messenger: Arc<PeerMessenger>,
) -> Box<dyn BlockManagement> {
    match config {
        AvalancheConfig::Basic { config } => {
            Box::new(AvalancheBlockManagement::new(id, config, peer_messenger))
        }
        AvalancheConfig::Blizzard { config } => {
            Box::new(AvalancheBlockManagement::new(id, config, peer_messenger))
        }
        AvalancheConfig::VoteNo { config } => {
            Box::new(AvalancheVoteNoBlockManagement::new(id, config))
        }
    }
}
