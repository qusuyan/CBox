use super::BlockDissemination;
use crate::peers::PeerMessenger;
use crate::protocol::block::Block;
use crate::utils::{CopycatError, NodeId};

use async_trait::async_trait;

use std::sync::Arc;

pub struct PassthroughBlockDissemination {
    _me: NodeId,
}

impl PassthroughBlockDissemination {
    pub fn new(me: NodeId, _peer_messenger: Arc<PeerMessenger>) -> Self {
        Self { _me: me }
    }
}

#[async_trait]
impl BlockDissemination for PassthroughBlockDissemination {
    async fn disseminate(&self, _src: NodeId, _blk: Arc<Block>) -> Result<(), CopycatError> {
        Ok(())
    }
}
