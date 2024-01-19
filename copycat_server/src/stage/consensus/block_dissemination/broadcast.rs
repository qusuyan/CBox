use super::BlockDissemination;
use crate::peers::PeerMessenger;
use copycat_protocol::block::Block;
use copycat_protocol::MsgType;
use copycat_utils::{CopycatError, NodeId};

use async_trait::async_trait;

use std::sync::Arc;

pub struct BroadcastBlockDissemination {
    me: NodeId,
    peer_messenger: Arc<PeerMessenger>,
}

impl BroadcastBlockDissemination {
    pub fn new(me: NodeId, peer_messenger: Arc<PeerMessenger>) -> Self {
        Self { me, peer_messenger }
    }
}

#[async_trait]
impl BlockDissemination for BroadcastBlockDissemination {
    async fn disseminate(&self, src: NodeId, blk: &Block) -> Result<(), CopycatError> {
        if self.me == src {
            self.peer_messenger
                .broadcast(MsgType::NewBlock { blk: blk.clone() })
                .await?;
        }
        Ok(())
    }
}
