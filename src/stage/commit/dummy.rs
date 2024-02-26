use super::Commit;
use crate::protocol::block::Block;
use crate::protocol::transaction::Txn;
use crate::utils::CopycatError;

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::mpsc;

pub struct DummyCommit {
    executed_send: mpsc::Sender<Arc<Txn>>,
}

impl DummyCommit {
    pub fn new(executed_send: mpsc::Sender<Arc<Txn>>) -> Self {
        Self { executed_send }
    }
}

#[async_trait]
impl Commit for DummyCommit {
    async fn commit(&self, block: Arc<Block>) -> Result<(), CopycatError> {
        for txn in &block.txns {
            if let Err(e) = self.executed_send.send(txn.clone()).await {
                return Err(CopycatError(e.to_string()));
            }
        }

        Ok(())
    }
}
