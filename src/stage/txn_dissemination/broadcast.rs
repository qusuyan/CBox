use super::TxnDissemination;
use crate::peers::PeerMessenger;
use crate::protocol::transaction::Txn;
use crate::protocol::MsgType;
use crate::utils::{CopycatError, NodeId};

use async_trait::async_trait;

use std::sync::Arc;

pub struct BroadcastTxnDissemination {
    me: NodeId,
    transport: Arc<PeerMessenger>,
}

impl BroadcastTxnDissemination {
    pub fn new(me: NodeId, transport: Arc<PeerMessenger>) -> Self {
        Self { me, transport }
    }
}

#[async_trait]
impl TxnDissemination for BroadcastTxnDissemination {
    async fn disseminate(&self, src: NodeId, txn: &Txn) -> Result<(), CopycatError> {
        // only the node that receives the txn first needs to broadcast
        if src == self.me {
            return self
                .transport
                .broadcast(MsgType::NewTxn { txn: txn.clone() })
                .await;
        }
        Ok(())
    }
}
