use super::Pacemaker;
use crate::utils::CopycatError;
use crate::NodeId;

use async_trait::async_trait;

use tokio::sync::Notify;

pub struct DummyPacemaker {
    _notify: Notify,
}

impl DummyPacemaker {
    pub fn new() -> Self {
        Self {
            _notify: Notify::new(),
        }
    }
}

#[async_trait]
impl Pacemaker for DummyPacemaker {
    async fn wait_to_propose(&self) -> Result<(), CopycatError> {
        self._notify.notified().await;
        Ok(())
    }

    async fn get_propose_msg(&mut self) -> Result<Vec<u8>, CopycatError> {
        unreachable!();
    }

    async fn handle_feedback(&mut self, _feedback: Vec<u8>) -> Result<(), CopycatError> {
        unreachable!();
    }

    async fn handle_peer_msg(&mut self, _src: NodeId, _msg: Vec<u8>) -> Result<(), CopycatError> {
        unreachable!();
    }
}
