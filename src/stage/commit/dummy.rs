use super::Commit;
use crate::protocol::block::Block;
use crate::utils::CopycatError;

use async_trait::async_trait;

use std::sync::Arc;

pub struct DummyCommit {}

impl DummyCommit {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Commit for DummyCommit {
    async fn commit(&self, _block: Arc<Block>) -> Result<(), CopycatError> {
        Ok(())
    }
}
