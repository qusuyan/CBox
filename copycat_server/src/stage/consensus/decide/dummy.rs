use async_trait::async_trait;

use super::Decision;
use copycat_utils::CopycatError;

pub struct DummyDecision {}

impl DummyDecision {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl<BlockType> Decision<BlockType> for DummyDecision {
    async fn decide(&self, _block: &BlockType) -> Result<bool, CopycatError> {
        Ok(true)
    }
}
