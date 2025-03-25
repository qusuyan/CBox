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

pub struct SamplingBlockDissemination {
    me: NodeId,
    peer_messenger: Arc<PeerMessenger>,
    neighbors: usize,
    tails: VecDeque<(NodeId, Vec<(Arc<Block>, Arc<BlkCtx>)>)>,
    _notify: Notify,
}

impl SamplingBlockDissemination {
    pub fn new(me: NodeId, peer_messenger: Arc<PeerMessenger>, sample_size: usize) -> Self {
        Self {
            me,
            peer_messenger,
            neighbors: sample_size - 1, // do not count self
            tails: VecDeque::new(),
            _notify: Notify::new(),
        }
    }
}

#[async_trait]
impl BlockDissemination for SamplingBlockDissemination {
    async fn disseminate(
        &mut self,
        src: NodeId,
        blocks: Vec<(Arc<Block>, Arc<BlkCtx>)>,
    ) -> Result<(), CopycatError> {
        if src == self.me {
            let (blk, _) = blocks.last().unwrap();
            self.peer_messenger
                .sample(MsgType::NewBlock { blk: blk.clone() }, self.neighbors)
                .await?;
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
