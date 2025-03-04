use super::Pacemaker;
use crate::peers::PeerMessenger;
use crate::{CopycatError, NodeId};

use async_trait::async_trait;
use tokio::sync::Notify;

use std::collections::VecDeque;
use std::sync::Arc;

pub struct DiemPacemaker {
    id: NodeId,
    pending_qcs: VecDeque<Vec<u8>>,
    peer_messenger: Arc<PeerMessenger>,
    _notify: Notify,
}

impl DiemPacemaker {
    pub fn new(id: NodeId, peer_messenger: Arc<PeerMessenger>) -> Self {
        Self {
            id,
            pending_qcs: VecDeque::new(),
            peer_messenger,
            _notify: Notify::new(),
        }
    }

    // round robin TODO: add reputable leader selection
    pub fn get_leader(round: u64, all_nodes: &Vec<NodeId>) -> NodeId {
        let idx = (round / 2) % all_nodes.len() as u64;
        all_nodes[idx as usize]
    }
}

#[async_trait]
impl Pacemaker for DiemPacemaker {
    async fn wait_to_propose(&self) {
        loop {
            if !self.pending_qcs.is_empty() {
                return;
            }
            self._notify.notified().await;
        }
    }

    async fn get_propose_msg(&mut self) -> Result<Vec<u8>, CopycatError> {
        let qc = self.pending_qcs.pop_front().unwrap();
        Ok(qc)
    }

    async fn handle_feedback(&mut self, feedback: Vec<u8>) -> Result<(), CopycatError> {
        self.pending_qcs.push_back(feedback);
        Ok(())
    }

    async fn handle_peer_msg(&mut self, src: NodeId, msg: Vec<u8>) -> Result<(), CopycatError> {
        todo!()
    }
}
