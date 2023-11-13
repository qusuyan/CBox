use super::Decision;
use copycat_protocol::block::Block;
use copycat_utils::CopycatError;

use async_trait::async_trait;

pub struct DummyDecision {}

impl DummyDecision {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Decision for DummyDecision {
    async fn decide(&self, _block: &Block) -> Result<bool, CopycatError> {
        Ok(true)
    }
}
