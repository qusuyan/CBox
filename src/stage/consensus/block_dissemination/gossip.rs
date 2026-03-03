use super::BlockDissemination;
use crate::context::BlkCtx;
use crate::peers::PeerMessenger;
use crate::protocol::block::Block;
use crate::protocol::MsgType;
use crate::utils::{CopycatError, NodeId};

use async_trait::async_trait;

use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use tokio::sync::Notify;

pub struct GossipBlockDissemination {
    me: NodeId,
    peer_messenger: Arc<PeerMessenger>,
    tails: VecDeque<(NodeId, Vec<(Arc<Block>, Arc<BlkCtx>)>)>,
    _notify: Notify,
}

impl GossipBlockDissemination {
    pub fn new(me: NodeId, peer_messenger: Arc<PeerMessenger>) -> Self {
        Self {
            me,
            peer_messenger,
            tails: VecDeque::new(),
            _notify: Notify::new(),
        }
    }
}

#[async_trait]
impl BlockDissemination for GossipBlockDissemination {
    async fn disseminate(
        &mut self,
        src: NodeId,
        blocks: Vec<(Arc<Block>, Arc<BlkCtx>)>,
    ) -> Result<(), CopycatError> {
        let (blk, _) = blocks.last().unwrap();
        self.peer_messenger
            .gossip(
                Box::new(MsgType::NewBlock { blk: blk.clone() }),
                HashSet::from([src, self.me]),
            )
            .await?;
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
