use super::BlockDissemination;
use crate::peers::PeerMessenger;
use crate::protocol::block::Block;
use crate::protocol::MsgType;
use crate::utils::{CopycatError, NodeId};

use async_trait::async_trait;

use std::sync::Arc;

pub struct LinearBlockDissemination {
    peer_messenger: Arc<PeerMessenger>,
    prev: NodeId,
    next: Option<NodeId>,
}

impl LinearBlockDissemination {
    pub fn new(me: NodeId, peer_messenger: Arc<PeerMessenger>, rep_order: Vec<NodeId>) -> Self {
        let mut idx = 0;
        let (prev, next) = loop {
            assert!(idx < rep_order.len());
            if rep_order[idx] == me {
                let prev = if idx == 0 {
                    rep_order[0]
                } else {
                    rep_order[idx - 1]
                };
                let next = if idx == rep_order.len() - 1 {
                    None
                } else {
                    Some(rep_order[idx + 1])
                };
                break (prev, next);
            }
            idx += 1;
        };

        Self {
            peer_messenger,
            prev,
            next,
        }
    }
}

#[async_trait]
impl BlockDissemination for LinearBlockDissemination {
    async fn disseminate(&mut self, src: NodeId, blk: Arc<Block>) -> Result<(), CopycatError> {
        assert!(src == self.prev);
        match self.next {
            Some(next) => {
                self.peer_messenger
                    .send(next, MsgType::NewBlock { blk })
                    .await?
            }
            None => {}
        }

        Ok(())
    }

    async fn handle_peer_msg(&mut self, _src: NodeId, _msg: Vec<u8>) -> Result<(), CopycatError> {
        unreachable!()
    }
}
