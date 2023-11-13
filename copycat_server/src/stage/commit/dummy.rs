use super::Commit;
use copycat_protocol::block::Block;
use copycat_protocol::transaction::Txn;
use copycat_utils::CopycatError;

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
        for txn in block.fetch_txns() {
            if let Err(e) = self.executed_send.send(txn).await {
                return Err(CopycatError(e.to_string()));
            }
        }

        Ok(())
    }
}
