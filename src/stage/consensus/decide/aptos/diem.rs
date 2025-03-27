use crate::config::AptosDiemConfig;
use crate::context::BlkCtx;
use crate::peers::PeerMessenger;
use crate::protocol::block::{Block, BlockHeader};
use crate::protocol::crypto::signature::P2PSignature;
use crate::protocol::crypto::threshold_signature::ThresholdSignature;
use crate::protocol::crypto::{PrivKey, PubKey};
use crate::protocol::types::aptos::CoA;
use crate::protocol::types::diem::{DiemBlock, TimeCert};
use crate::stage::consensus::decide::Decision;
use crate::stage::DelayPool;
use crate::transaction::Txn;
use crate::{CopycatError, NodeId, SignatureScheme};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
enum DiemConsensusMsg {
    Proposal {
        block: DiemBlock,
        last_round_tc: Option<TimeCert>,
    },
}

pub struct AptosDiemDecision {
    id: NodeId,
    blk_len: usize,
    _proposao_timeout: Duration,
    _vote_timeout_secs: Duration, // TODO
    //
    quorum_store: HashMap<(NodeId, u64), (Arc<Block>, Arc<BlkCtx>)>,
    // P2P communication
    all_nodes: Vec<NodeId>,
    peer_messenger: Arc<PeerMessenger>,
    signature_scheme: SignatureScheme,
    _peer_pks: HashMap<NodeId, PubKey>,
    sk: PrivKey,
    threshold_signature: Arc<dyn ThresholdSignature>,
    //
    pmaker_feedback_send: mpsc::Sender<Vec<u8>>,
    delay: Arc<DelayPool>,
}

impl AptosDiemDecision {
    pub fn new(
        id: NodeId,
        p2p_signature: P2PSignature,
        threshold_signature: Arc<dyn ThresholdSignature>,
        config: AptosDiemConfig,
        peer_messenger: Arc<PeerMessenger>,
        pmaker_feedback_send: mpsc::Sender<Vec<u8>>,
        delay: Arc<DelayPool>,
    ) -> Self {
        let (signature_scheme, peer_pks, sk) = p2p_signature;

        let mut all_nodes: Vec<NodeId> = peer_pks.keys().cloned().collect();
        all_nodes.sort();

        Self {
            id,
            blk_len: config.diem_blk_len,
            _proposao_timeout: Duration::from_secs_f64(config.diem_batching_timeout_secs),
            _vote_timeout_secs: Duration::from_secs_f64(config.diem_vote_timeout_secs),
            quorum_store: HashMap::new(),
            all_nodes,
            peer_messenger,
            signature_scheme,
            _peer_pks: peer_pks,
            sk,
            threshold_signature,
            pmaker_feedback_send,
            delay,
        }
    }
}

#[async_trait]
impl Decision for AptosDiemDecision {
    async fn new_tail(
        &mut self,
        _src: NodeId,
        new_tail: Vec<(Arc<Block>, Arc<BlkCtx>)>,
    ) -> Result<(), CopycatError> {
        for (blk, ctx) in new_tail {
            let (sender, round) = match blk.header {
                BlockHeader::Aptos { sender, round, .. } => (sender, round),
                _ => unreachable!(),
            };
            self.quorum_store.insert((sender, round), (blk, ctx));
        }
        Ok(())
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
