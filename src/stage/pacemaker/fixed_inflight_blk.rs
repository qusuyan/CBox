use super::Pacemaker;
use crate::{CopycatError, NodeId};

use async_trait::async_trait;
use tokio::sync::Notify;

pub struct FixedInflightBlkPacemaker {
    quota: usize,
    _notify: Notify,
}

impl FixedInflightBlkPacemaker {
    pub fn new(max_inflight_blk: usize) -> Self {
        Self {
            quota: max_inflight_blk,
            _notify: Notify::new(),
        }
    }
}

#[async_trait]
impl Pacemaker for FixedInflightBlkPacemaker {
    async fn wait_to_propose(&self) {
        if self.quota == 0 {
            self._notify.notified().await;
        }
    }

    async fn get_propose_msg(&mut self) -> Result<Vec<u8>, CopycatError> {
        self.quota -= 1;
        Ok(vec![])
    }

    async fn handle_feedback(&mut self, _feedback: Vec<u8>) -> Result<(), CopycatError> {
        self.quota += 1;
        Ok(())
    }

    async fn handle_peer_msg(&mut self, _src: NodeId, _msg: Vec<u8>) -> Result<(), CopycatError> {
        unreachable!();
    }
}
