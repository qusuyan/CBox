use super::Pacemaker;
use crate::{config::AptosDiemConfig, CopycatError, NodeId};

use async_trait::async_trait;

pub struct AptosPmaker {
    id: NodeId,
}

impl AptosPmaker {
    pub fn new(id: NodeId, config: AptosDiemConfig) -> Self {
        Self { id }
    }
}

#[async_trait]
impl Pacemaker for AptosPmaker {
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
