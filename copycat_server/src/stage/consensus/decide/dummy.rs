use super::Decision;
use copycat_protocol::block::Block;
use copycat_utils::CopycatError;

use async_trait::async_trait;

use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Notify;

pub struct DummyDecision {
    blocks_to_commit: VecDeque<Arc<Block>>,
    notify: Notify,
}

impl DummyDecision {
    pub fn new() -> Self {
        Self {
            blocks_to_commit: VecDeque::new(),
            notify: Notify::new(),
        }
    }
}

#[async_trait]
impl Decision for DummyDecision {
    async fn new_tail(&mut self, new_tail: Vec<Arc<Block>>) -> Result<(), CopycatError> {
        self.blocks_to_commit.append(&mut VecDeque::from(new_tail));
        Ok(())
    }

    async fn commit_ready(&self) -> Result<(), CopycatError> {
        loop {
            if !self.blocks_to_commit.is_empty() {
                return Ok(());
            }
            self.notify.notified().await;
        }
    }

    async fn next_to_commit(&mut self) -> Result<Arc<Block>, CopycatError> {
        match self.blocks_to_commit.pop_front() {
            Some(block) => Ok(block),
            None => Err(CopycatError(String::from("no blocks to commit"))),
        }
    }
}
