use super::TxnDissemination;
use crate::protocol::transaction::Txn;
use crate::utils::{CopycatError, NodeId};

use async_trait::async_trait;

use std::sync::Arc;
pub struct PassthroughTxnDissemination {}

impl PassthroughTxnDissemination {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl TxnDissemination for PassthroughTxnDissemination {
    async fn disseminate(&self, _txn_batch: &Vec<(NodeId, Arc<Txn>)>) -> Result<(), CopycatError> {
        // do nothing
        Ok(())
    }
}
