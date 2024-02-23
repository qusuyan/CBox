use super::BlockDissemination;
use crate::peers::PeerMessenger;
use copycat_protocol::block::Block;
use copycat_protocol::MsgType;
use copycat_utils::{CopycatError, NodeId};

use async_trait::async_trait;

use std::{collections::HashSet, sync::Arc};

pub struct GossipBlockDissemination {
    me: NodeId,
    peer_messenger: Arc<PeerMessenger>,
}

impl GossipBlockDissemination {
    pub fn new(me: NodeId, peer_messenger: Arc<PeerMessenger>) -> Self {
        Self { me, peer_messenger }
    }
}

#[async_trait]
impl BlockDissemination for GossipBlockDissemination {
    async fn disseminate(&self, src: NodeId, blk: &Block) -> Result<(), CopycatError> {
        self.peer_messenger
            .gossip(
                MsgType::NewBlock { blk: blk.clone() },
                HashSet::from([src, self.me]),
            )
            .await
    }
}
