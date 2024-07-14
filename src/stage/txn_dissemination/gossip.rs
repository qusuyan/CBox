use super::TxnDissemination;
use crate::peers::PeerMessenger;
use crate::protocol::transaction::Txn;
use crate::protocol::MsgType;
use crate::utils::{CopycatError, NodeId};

use async_trait::async_trait;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub struct GossipTxnDissemination {
    me: NodeId,
    transport: Arc<PeerMessenger>,
}

impl GossipTxnDissemination {
    pub fn new(me: NodeId, transport: Arc<PeerMessenger>) -> Self {
        Self { me, transport }
    }
}

#[async_trait]
impl TxnDissemination for GossipTxnDissemination {
    async fn disseminate(&self, txn_batch: &Vec<(NodeId, Arc<Txn>)>) -> Result<(), CopycatError> {
        // gossip to all neighbors except where it comes from
        let mut txn_batches_by_sender = HashMap::new();
        for (src, txn) in txn_batch {
            let dst_batch = txn_batches_by_sender.entry(src).or_insert(vec![]);
            dst_batch.push(txn.clone());
        }

        for (src, txn_batch) in txn_batches_by_sender {
            if txn_batch.len() > 0 {
                self.transport
                    .gossip(
                        MsgType::NewTxn { txn_batch },
                        HashSet::from([*src, self.me]),
                    )
                    .await?;
            }
        }
        Ok(())
    }
}
