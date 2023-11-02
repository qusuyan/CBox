use async_trait::async_trait;
use copycat_utils::CopycatError;
use serde::{de::DeserializeOwned, Serialize};

use std::sync::Arc;

use super::TxnDissemination;
use crate::peers::{MsgType, PeerMessenger};

pub struct BroadcastTxnDissemination<TxnType, BlockType> {
    transport: Arc<PeerMessenger<TxnType, BlockType>>,
}

impl<TxnType, BlockType> BroadcastTxnDissemination<TxnType, BlockType> {
    pub fn new(transport: Arc<PeerMessenger<TxnType, BlockType>>) -> Self {
        Self { transport }
    }
}

#[async_trait]
impl<TxnType, BlockType> TxnDissemination<TxnType> for BroadcastTxnDissemination<TxnType, BlockType>
where
    TxnType: 'static + std::fmt::Debug + Serialize + DeserializeOwned + Sync + Send,
    BlockType: 'static + std::fmt::Debug + Serialize + DeserializeOwned + Sync + Send,
{
    async fn disseminate(&self, txn: TxnType) -> Result<(), CopycatError> {
        self.transport.broadcast(MsgType::NewTxn { txn }).await
    }
}
