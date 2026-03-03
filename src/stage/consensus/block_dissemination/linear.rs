use super::BlockDissemination;
use crate::context::BlkCtx;
use crate::peers::PeerMessenger;
use crate::protocol::block::Block;
use crate::protocol::MsgType;
use crate::utils::{CopycatError, NodeId};

use async_trait::async_trait;

use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Notify;

pub struct LinearBlockDissemination {
    peer_messenger: Arc<PeerMessenger>,
    prev: NodeId,
    next: Option<NodeId>,
    tails: VecDeque<(NodeId, Vec<(Arc<Block>, Arc<BlkCtx>)>)>,
    _notify: Notify,
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
            tails: VecDeque::new(),
            _notify: Notify::new(),
        }
    }
}

#[async_trait]
impl BlockDissemination for LinearBlockDissemination {
    async fn disseminate(
        &mut self,
        src: NodeId,
        blocks: Vec<(Arc<Block>, Arc<BlkCtx>)>,
    ) -> Result<(), CopycatError> {
        assert!(src == self.prev);
        match self.next {
            Some(next) => {
                let (blk, _) = blocks.last().unwrap();
                self.peer_messenger
                    .send(next, Box::new(MsgType::NewBlock { blk: blk.clone() }))
                    .await?
            }
            None => {}
        }
        self.tails.push_back((src, blocks));
        Ok(())
    }

    async fn wait_disseminated(&self) -> Result<(), CopycatError> {
        if self.tails.is_empty() {
            // will wait forever
            self._notify.notified().await;
        }
        Ok(())
    }

    async fn get_disseminated(
        &mut self,
    ) -> Result<(NodeId, Vec<(Arc<Block>, Arc<BlkCtx>)>), CopycatError> {
        Ok(self.tails.pop_front().unwrap())
    }

    async fn handle_peer_msg(&mut self, _src: NodeId, _msg: Vec<u8>) -> Result<(), CopycatError> {
        unreachable!()
    }
}
