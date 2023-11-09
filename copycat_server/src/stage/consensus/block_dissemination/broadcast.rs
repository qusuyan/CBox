use async_trait::async_trait;
use copycat_utils::CopycatError;
use serde::{de::DeserializeOwned, Serialize};

use std::sync::Arc;

use super::BlockDissemination;
use crate::peers::PeerMessenger;
use copycat_protocol::MsgType;

pub struct BroadcastBlockDissemination<TxnType, BlockType> {
    peer_messenger: Arc<PeerMessenger<TxnType, BlockType>>,
}

impl<TxnType, BlockType> BroadcastBlockDissemination<TxnType, BlockType> {
    pub fn new(peer_messenger: Arc<PeerMessenger<TxnType, BlockType>>) -> Self {
        Self { peer_messenger }
    }
}

#[async_trait]
impl<TxnType, BlockType> BlockDissemination<BlockType>
    for BroadcastBlockDissemination<TxnType, BlockType>
where
    TxnType: 'static + std::fmt::Debug + Serialize + DeserializeOwned + Sync + Send,
    BlockType: 'static + std::fmt::Debug + Clone + Serialize + DeserializeOwned + Sync + Send,
{
    async fn disseminate(&self, blk: &BlockType) -> Result<(), CopycatError> {
        self.peer_messenger
            .broadcast(MsgType::NewBlock { blk: blk.clone() })
            .await
    }
}
