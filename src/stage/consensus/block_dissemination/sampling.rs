use super::BlockDissemination;
use crate::peers::PeerMessenger;
use crate::protocol::block::Block;
use crate::protocol::MsgType;
use crate::utils::{CopycatError, NodeId};
use crate::Config;

use async_trait::async_trait;

use std::sync::Arc;

pub struct SamplingBlockDissemination {
    me: NodeId,
    peer_messenger: Arc<PeerMessenger>,
    neighbors: usize,
}

impl SamplingBlockDissemination {
    pub fn new(me: NodeId, peer_messenger: Arc<PeerMessenger>, config: Config) -> Self {
        let sample_size = match config {
            Config::Avalanche { config } => config.k,
            _ => unimplemented!(),
        };
        Self {
            me,
            peer_messenger,
            neighbors: sample_size - 1, // get rid of self
        }
    }
}

#[async_trait]
impl BlockDissemination for SamplingBlockDissemination {
    async fn disseminate(&self, src: NodeId, blk: &Block) -> Result<(), CopycatError> {
        if src == self.me {
            self.peer_messenger
                .sample(MsgType::NewBlock { blk: blk.clone() }, self.neighbors)
                .await?;
        }
        Ok(())
    }
}
