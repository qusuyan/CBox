mod correct;
use blizzard::BlizzardDecision;
use correct::AvalancheDecision;

mod blizzard;

mod vote_no;
use vote_no::AvalancheVoteNoDecision;

use super::Decision;
use crate::config::AvalancheConfig;
use crate::peers::PeerMessenger;
use crate::protocol::crypto::Signature;
use crate::{CryptoScheme, NodeId};

use atomic_float::AtomicF64;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;

#[derive(Serialize, Deserialize, Debug)]
struct VoteMsg {
    pub round: (NodeId, u64),
    pub votes: Vec<bool>,
    pub signature: Signature,
}

pub fn new(
    id: NodeId,
    crypto_scheme: CryptoScheme,
    config: AvalancheConfig,
    peer_messenger: Arc<PeerMessenger>,
    pmaker_feedback_send: mpsc::Sender<Vec<u8>>,
    delay: Arc<AtomicF64>,
) -> Box<dyn Decision> {
    match config {
        AvalancheConfig::Correct { config } => Box::new(AvalancheDecision::new(
            id,
            crypto_scheme,
            config,
            peer_messenger,
            pmaker_feedback_send,
            delay,
        )),
        AvalancheConfig::Blizzard { config } => Box::new(BlizzardDecision::new(
            id,
            crypto_scheme,
            config,
            peer_messenger,
            pmaker_feedback_send,
            delay,
        )),
        AvalancheConfig::VoteNo { config } => Box::new(AvalancheVoteNoDecision::new(
            id,
            crypto_scheme,
            config,
            peer_messenger,
            delay,
        )),
    }
}
