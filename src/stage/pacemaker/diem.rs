use super::Pacemaker;
use crate::peers::PeerMessenger;
use crate::{CopycatError, NodeId};

use async_trait::async_trait;

use std::sync::Arc;

pub struct DiemPacemaker {
    id: NodeId,
    peer_messenger: Arc<PeerMessenger>,
}

impl DiemPacemaker {
    pub fn new(id: NodeId, peer_messenger: Arc<PeerMessenger>) -> Self {
        Self { id, peer_messenger }
    }

    pub fn get_leader(round: u64) -> NodeId {
        todo!();
    }
}

#[async_trait]
impl Pacemaker for DiemPacemaker {
    async fn wait_to_propose(&self) {
        todo!()
    }

    async fn get_propose_msg(&mut self) -> Result<Vec<u8>, CopycatError> {
        todo!()
    }

    async fn handle_feedback(&mut self, feedback: Vec<u8>) -> Result<(), CopycatError> {
        todo!()
    }

    async fn handle_peer_msg(&mut self, src: NodeId, msg: Vec<u8>) -> Result<(), CopycatError> {
        todo!()
    }
}
