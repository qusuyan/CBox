use async_trait::async_trait;

use super::BlockValidation;
use copycat_utils::CopycatError;

pub struct DummyBlockValidation {}

impl DummyBlockValidation {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl<BlockType> BlockValidation<BlockType> for DummyBlockValidation {
    async fn validate(&self, _block: &BlockType) -> Result<bool, CopycatError> {
        Ok(true)
    }
}
