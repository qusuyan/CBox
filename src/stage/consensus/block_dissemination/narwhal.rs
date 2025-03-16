use super::BlockDissemination;
use crate::protocol::block::Block;
use crate::{CopycatError, NodeId};

use async_trait::async_trait;

use std::sync::Arc;

pub struct Narwhal {}

impl Narwhal {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl BlockDissemination for Narwhal {
    async fn disseminate(&mut self, src: NodeId, block: Arc<Block>) -> Result<(), CopycatError> {
        todo!()
    }

    async fn handle_peer_msg(&mut self, src: NodeId, msg: Vec<u8>) -> Result<(), CopycatError> {
        todo!()
    }
}
