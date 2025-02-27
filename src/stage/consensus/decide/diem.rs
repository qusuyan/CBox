use super::Decision;
use crate::config::DiemConfig;
use crate::context::BlkCtx;
use crate::peers::PeerMessenger;
use crate::protocol::block::Block;
use crate::transaction::Txn;
use crate::{CopycatError, NodeId, SignatureScheme};

use std::sync::Arc;
use tokio::sync::mpsc;

use async_trait::async_trait;
use atomic_float::AtomicF64;

pub struct DiemDecision {
    id: NodeId,
    signature_scheme: SignatureScheme,
    proposal_timeout_secs: f64,
    vote_timeout_secs: f64,
    peer_messenger: Arc<PeerMessenger>,
    pmaker_feedback_send: mpsc::Sender<Vec<u8>>,
    delay: Arc<AtomicF64>,
}

impl DiemDecision {
    pub fn new(
        id: NodeId,
        signature_scheme: SignatureScheme,
        config: DiemConfig,
        peer_messenger: Arc<PeerMessenger>,
        pmaker_feedback_send: mpsc::Sender<Vec<u8>>,
        delay: Arc<AtomicF64>,
    ) -> Self {
        Self {
            id,
            signature_scheme,
            proposal_timeout_secs: config.proposal_timeout_secs,
            vote_timeout_secs: config.vote_timeout_secs,
            peer_messenger,
            pmaker_feedback_send,
            delay,
        }
    }
}

#[async_trait]
impl Decision for DiemDecision {
    async fn new_tail(
        &mut self,
        src: NodeId,
        new_tail: Vec<(Arc<Block>, Arc<BlkCtx>)>,
    ) -> Result<(), CopycatError> {
        todo!()
    }

    async fn commit_ready(&self) -> Result<(), CopycatError> {
        todo!()
    }

    async fn next_to_commit(&mut self) -> Result<(u64, Vec<Arc<Txn>>), CopycatError> {
        todo!()
    }

    async fn timeout(&self) -> Result<(), CopycatError> {
        todo!()
    }

    async fn handle_timeout(&mut self) -> Result<(), CopycatError> {
        todo!()
    }

    async fn handle_peer_msg(&mut self, src: NodeId, content: Vec<u8>) -> Result<(), CopycatError> {
        todo!()
    }

    fn report(&mut self) {}
}
