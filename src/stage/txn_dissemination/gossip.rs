use super::TxnDissemination;
use crate::peers::PeerMessenger;
use crate::protocol::transaction::Txn;
use crate::protocol::MsgType;
use crate::utils::{CopycatError, NodeId};

use async_trait::async_trait;

use std::{collections::HashSet, sync::Arc};

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
    async fn disseminate(&self, src: NodeId, txn: &Txn) -> Result<(), CopycatError> {
        // gossip to all neighbors except where it comes from
        self.transport
            .gossip(
                MsgType::NewTxn { txn: txn.clone() },
                HashSet::from([src, self.me]),
            )
            .await
    }
}
