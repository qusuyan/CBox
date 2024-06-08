use super::BlockDissemination;
use crate::peers::PeerMessenger;
use crate::protocol::block::Block;
use crate::protocol::MsgType;
use crate::utils::{CopycatError, NodeId};
use crate::Config;

use async_trait::async_trait;

use std::sync::Arc;

pub struct LinearBlockDissemination {
    peer_messenger: Arc<PeerMessenger>,
    prev: NodeId,
    next: Option<NodeId>,
}

impl LinearBlockDissemination {
    pub fn new(me: NodeId, peer_messenger: Arc<PeerMessenger>, config: Config) -> Self {
        let rep_order = match config {
            Config::ChainReplication { config } => config.order,
            _ => unimplemented!(),
        };

        let idx = 0;
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
    async fn disseminate(&self, src: NodeId, blk: &Block) -> Result<(), CopycatError> {
        assert!(src == self.prev);
        match self.next {
            Some(next) => {
                self.peer_messenger
                    .send(next, MsgType::NewBlock { blk: blk.clone() })
                    .await?
            }
            None => {}
        }

        Ok(())
    }
}
