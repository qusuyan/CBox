use async_trait::async_trait;

use super::Commit;
use crate::block::BlockTrait;
use copycat_utils::CopycatError;

use std::sync::Arc;
use tokio::sync::mpsc;

pub struct DummyCommit<TxnType> {
    executed_send: mpsc::Sender<Arc<TxnType>>,
}

impl<TxnType> DummyCommit<TxnType> {
    pub fn new(executed_send: mpsc::Sender<Arc<TxnType>>) -> Self {
        Self { executed_send }
    }
}

#[async_trait]
impl<TxnType> Commit<dyn BlockTrait<TxnType>> for DummyCommit<TxnType>
where
    TxnType: Sync + Send,
{
    async fn commit(&self, block: Arc<dyn BlockTrait<TxnType>>) -> Result<(), CopycatError> {
        for txn in block.fetch_txns() {
            if let Err(e) = self.executed_send.send(txn).await {
                return Err(CopycatError(e.to_string()));
            }
        }

        Ok(())
    }
}
