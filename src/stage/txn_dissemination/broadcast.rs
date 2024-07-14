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
    async fn disseminate(&self, txn_batch: &Vec<(NodeId, Arc<Txn>)>) -> Result<(), CopycatError> {
        // only the node that receives the txn first needs to broadcast
        let txn_to_dissem: Vec<Arc<Txn>> = txn_batch
            .iter()
            .filter(|(src, _)| *src == self.me)
            .map(|(_, txn)| txn.clone())
            .collect();
        if txn_to_dissem.len() > 0 {
            self.transport
                .broadcast(MsgType::NewTxn {
                    txn_batch: txn_to_dissem,
                })
                .await?;
        }
        Ok(())
    }
}
