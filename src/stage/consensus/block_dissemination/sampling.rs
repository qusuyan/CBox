use super::BlockDissemination;
use crate::peers::PeerMessenger;
use crate::protocol::block::Block;
use crate::protocol::MsgType;
use crate::utils::{CopycatError, NodeId};

use async_trait::async_trait;

use std::sync::Arc;

pub struct SamplingBlockDissemination {
    me: NodeId,
    peer_messenger: Arc<PeerMessenger>,
    neighbors: usize,
}

impl SamplingBlockDissemination {
    pub fn new(me: NodeId, peer_messenger: Arc<PeerMessenger>, sample_size: usize) -> Self {
        Self {
            me,
            peer_messenger,
            neighbors: sample_size - 1, // do not count self
        }
    }
}

#[async_trait]
impl BlockDissemination for SamplingBlockDissemination {
    async fn disseminate(&self, src: NodeId, blk: Arc<Block>) -> Result<(), CopycatError> {
        if src == self.me {
            self.peer_messenger
                .sample(MsgType::NewBlock { blk }, self.neighbors)
                .await?;
        }
        Ok(())
    }
}
