use super::TxnDissemination;
use crate::peers::PeerMessenger;
use copycat_protocol::transaction::Txn;
use copycat_protocol::MsgType;
use copycat_utils::CopycatError;

use async_trait::async_trait;

use std::sync::Arc;

pub struct BroadcastTxnDissemination {
    transport: Arc<PeerMessenger>,
}

impl BroadcastTxnDissemination {
    pub fn new(transport: Arc<PeerMessenger>) -> Self {
        Self { transport }
    }
}

#[async_trait]
impl TxnDissemination for BroadcastTxnDissemination {
    async fn disseminate(&self, txn: &Txn) -> Result<(), CopycatError> {
        self.transport
            .broadcast(MsgType::NewTxn { txn: txn.clone() })
            .await
    }
}
