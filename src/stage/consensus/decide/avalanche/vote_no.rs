use super::{Decision, VoteMsg};
use crate::config::AvalancheVoteNoConfig;
use crate::context::BlkCtx;
use crate::peers::PeerMessenger;
use crate::protocol::block::{Block, BlockHeader};
use crate::protocol::crypto::signature::P2PSignature;
use crate::protocol::crypto::PrivKey;
use crate::protocol::MsgType;
use crate::stage::DelayPool;
use crate::transaction::Txn;
use crate::{CopycatError, NodeId, SignatureScheme};

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::Notify;
use tokio::time::Duration;

pub struct AvalancheVoteNoDecision {
    id: NodeId,
    signature_scheme: SignatureScheme,
    sk: PrivKey,
    peer_messenger: Arc<PeerMessenger>,
    delay: Arc<DelayPool>,
    _notify: Notify,
}

impl AvalancheVoteNoDecision {
    pub fn new(
        id: NodeId,
        p2p_signature: P2PSignature,
        _config: AvalancheVoteNoConfig,
        peer_messenger: Arc<PeerMessenger>,
        delay: Arc<DelayPool>,
    ) -> Self {
        let (signature_scheme, _, sk) = p2p_signature;

        Self {
            id,
            signature_scheme,
            sk,
            peer_messenger,
            delay,
            _notify: Notify::new(),
        }
    }
}

#[async_trait]
impl Decision for AvalancheVoteNoDecision {
    async fn new_tail(
        &mut self,
        _src: NodeId,
        mut new_tail: Vec<(Arc<Block>, Arc<BlkCtx>)>,
    ) -> Result<(), CopycatError> {
        let (new_blk, _new_blk_ctx) = if new_tail.len() == 1 {
            new_tail.remove(0)
        } else {
            unreachable!("Avalanche blocks are in DAG not chain")
        };

        let (proposer, id) = match new_blk.header {
            BlockHeader::Avalanche { proposer, id, .. } => (proposer, id),
            _ => unreachable!(),
        };

        let votes = vec![false; new_blk.txns.len()];

        let msg_content = (id, votes);
        let serialized_msg = &bincode::serialize(&msg_content)?;
        let (signature, stime) = self.signature_scheme.sign(&self.sk, serialized_msg)?;
        self.delay.process_illusion(stime).await;
        let delay = self.delay.get_current_delay();
        let vote_msg = VoteMsg {
            round: msg_content.0,
            votes: msg_content.1,
            signature,
        };

        pf_debug!(self.id; "sending no votes for block {:?}", new_blk);

        self.peer_messenger
            .delayed_send(
                proposer,
                MsgType::ConsensusMsg {
                    msg: bincode::serialize(&vote_msg)?,
                },
                Duration::from_secs_f64(delay),
            )
            .await?;

        Ok(())
    }

    async fn commit_ready(&self) -> Result<(), CopycatError> {
        self._notify.notified().await;
        Ok(())
    }

    async fn next_to_commit(&mut self) -> Result<(u64, Vec<Arc<Txn>>), CopycatError> {
        unreachable!()
    }

    async fn handle_peer_msg(
        &mut self,
        _src: NodeId,
        _content: Vec<u8>,
    ) -> Result<(), CopycatError> {
        Ok(()) // ignore votes from peers
    }

    async fn timeout(&self) -> Result<(), CopycatError> {
        self._notify.notified().await;
        unreachable!();
    }

    async fn handle_timeout(&mut self) -> Result<(), CopycatError> {
        unreachable!();
    }

    fn report(&mut self) {}
}
