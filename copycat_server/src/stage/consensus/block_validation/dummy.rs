use super::BlockValidation;
use copycat_protocol::block::Block;
use copycat_utils::CopycatError;

use async_trait::async_trait;

pub struct DummyBlockValidation {}

impl DummyBlockValidation {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl BlockValidation for DummyBlockValidation {
    async fn validate(&self, _block: &Block) -> Result<bool, CopycatError> {
        Ok(true)
    }
}
