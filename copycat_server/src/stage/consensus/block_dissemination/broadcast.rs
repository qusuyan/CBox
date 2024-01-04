use super::BlockDissemination;
use crate::peers::PeerMessenger;
use copycat_protocol::block::Block;
use copycat_protocol::MsgType;
use copycat_utils::CopycatError;

use async_trait::async_trait;

use std::sync::Arc;

pub struct BroadcastBlockDissemination {
    peer_messenger: Arc<PeerMessenger>,
}

impl BroadcastBlockDissemination {
    pub fn new(peer_messenger: Arc<PeerMessenger>) -> Self {
        Self { peer_messenger }
    }
}

#[async_trait]
impl BlockDissemination for BroadcastBlockDissemination {
    async fn disseminate(&self, blk: &Block) -> Result<(), CopycatError> {
        log::debug!("broadcasting new block {blk:?}");
        self.peer_messenger
            .broadcast(MsgType::NewBlock { blk: blk.clone() })
            .await
    }
}
