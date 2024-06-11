use super::Decision;
use crate::context::BlkCtx;
use crate::peers::PeerMessenger;
use crate::protocol::block::Block;
use crate::protocol::crypto::Hash;
use crate::protocol::MsgType;
use crate::transaction::Txn;
use crate::{Config, CopycatError, NodeId};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Notify;

enum Role {
    Head,
    Intermediate,
    Tail,
}

#[derive(Serialize, Deserialize)]
struct Msg {
    recved: Hash,
}

pub struct ChainReplicationDecision {
    me: NodeId,
    role: Role,
    head: NodeId,
    tail: NodeId,
    peer_messenger: Arc<PeerMessenger>,
    inflight_blks: HashMap<Hash, Arc<Block>>,
    commit_ready: HashSet<Hash>,
    next_to_commit: Hash,
    _notify: Notify,
}

impl ChainReplicationDecision {
    pub fn new(me: NodeId, config: Config, peer_messenger: Arc<PeerMessenger>) -> Self {
        let rep_order = match config {
            Config::ChainReplication { config } => config.order,
            _ => unimplemented!(),
        };

        let head = *rep_order.first().unwrap();
        let tail = *rep_order.last().unwrap();
        let role = if head == me {
            Role::Head
        } else if tail == me {
            Role::Tail
        } else {
            Role::Intermediate
        };

        Self {
            me,
            role,
            head,
            tail,
            peer_messenger,
            inflight_blks: HashMap::new(),
            commit_ready: HashSet::new(),
            next_to_commit: Hash::one(),
            _notify: Notify::new(),
        }
    }
}

#[async_trait]
impl Decision for ChainReplicationDecision {
    async fn new_tail(
        &mut self,
        _src: NodeId,
        mut new_tail: Vec<(Arc<Block>, Arc<BlkCtx>)>,
    ) -> Result<(), CopycatError> {
        let (new_blk, new_blk_ctx) = if new_tail.len() < 1 {
            return Ok(());
        } else if new_tail.len() == 1 {
            new_tail.remove(0)
        } else {
            unreachable!("Avalanche blocks are in DAG not chain")
        };

        pf_debug!(self.me; "got new block: {:?}", new_blk);

        match self.role {
            Role::Head => {
                self.inflight_blks.insert(new_blk_ctx.id, new_blk);
            }
            Role::Tail => {
                let committed_msg = Msg {
                    recved: new_blk_ctx.id,
                };
                self.peer_messenger
                    .send(
                        self.head,
                        MsgType::ConsensusMsg {
                            msg: bincode::serialize(&committed_msg)?,
                        },
                    )
                    .await?;
            }
            Role::Intermediate => {}
        }

        Ok(())
    }

    async fn commit_ready(&self) -> Result<(), CopycatError> {
        loop {
            if self.commit_ready.contains(&self.next_to_commit) {
                return Ok(());
            }
            self._notify.notified().await;
        }
    }

    async fn next_to_commit(&mut self) -> Result<(u64, Vec<Arc<Txn>>), CopycatError> {
        assert!(self.commit_ready.remove(&self.next_to_commit));
        let blk_id = self.next_to_commit;
        let blk = self.inflight_blks.remove(&blk_id).unwrap();
        let txns = blk.txns.clone();
        self.next_to_commit += Hash::one();
        Ok((blk_id.as_u64(), txns))
    }

    async fn handle_peer_msg(&mut self, src: NodeId, content: Vec<u8>) -> Result<(), CopycatError> {
        assert!(matches!(self.role, Role::Head));
        assert!(src == self.tail);
        let msg: Msg = bincode::deserialize(&content)?;
        if self.inflight_blks.contains_key(&msg.recved) {
            self.commit_ready.insert(msg.recved);
        } else {
            pf_error!(self.me; "Received block that is not inflight: {}", msg.recved);
        }
        Ok(())
    }

    fn report(&mut self) {}
}
