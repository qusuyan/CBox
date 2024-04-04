use super::Decision;
use crate::protocol::block::Block;
use crate::transaction::Txn;
use crate::utils::CopycatError;
use crate::NodeId;

use async_trait::async_trait;

use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Notify;

pub struct DummyDecision {
    blocks_to_commit: VecDeque<Arc<Block>>,
    blocks_committed: u64,
    _notify: Notify,
}

impl DummyDecision {
    pub fn new() -> Self {
        Self {
            blocks_to_commit: VecDeque::new(),
            blocks_committed: 0,
            _notify: Notify::new(),
        }
    }
}

#[async_trait]
impl Decision for DummyDecision {
    async fn new_tail(
        &mut self,
        _src: NodeId,
        new_tail: Vec<Arc<Block>>,
    ) -> Result<(), CopycatError> {
        self.blocks_to_commit.append(&mut VecDeque::from(new_tail));
        Ok(())
    }

    async fn commit_ready(&self) -> Result<(), CopycatError> {
        loop {
            if !self.blocks_to_commit.is_empty() {
                return Ok(());
            }
            self._notify.notified().await;
        }
    }

    async fn next_to_commit(&mut self) -> Result<(u64, Vec<Arc<Txn>>), CopycatError> {
        match self.blocks_to_commit.pop_front() {
            Some(block) => {
                self.blocks_committed += 1;
                Ok((self.blocks_committed, block.txns.clone()))
            }
            None => Err(CopycatError(String::from("no blocks to commit"))),
        }
    }

    async fn handle_peer_msg(
        &mut self,
        _src: NodeId,
        _content: Arc<Vec<u8>>,
    ) -> Result<(), CopycatError> {
        unreachable!();
    }
}
