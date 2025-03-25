use super::BlockDissemination;
use crate::context::BlkCtx;
use crate::peers::PeerMessenger;
use crate::protocol::block::{Block, BlockHeader};
use crate::protocol::crypto::threshold_signature::{SignPart, ThresholdSignature};
use crate::protocol::crypto::{sha256, Hash};
use crate::protocol::types::aptos::CoA;
use crate::protocol::MsgType;
use crate::stage::DelayPool;
use crate::{CopycatError, NodeId};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::Notify;

#[derive(Clone, Debug, Serialize, Deserialize)]
enum NarwhalMsgs {
    Vote {
        sender: NodeId,
        round: u64,
        digest: Hash,
        signature: SignPart,
    },
}

pub struct NarwhalBlockDissemination {
    me: NodeId,
    blk_pool: HashMap<(NodeId, u64, Hash), (NodeId, (Arc<Block>, Arc<BlkCtx>))>,
    peer_messenger: Arc<PeerMessenger>,
    threshold_signature: Arc<dyn ThresholdSignature>,
    signatures: HashMap<(NodeId, u64, Hash), HashMap<NodeId, SignPart>>,
    ready_tails: VecDeque<(NodeId, u64, Hash)>,
    conflict_set: HashMap<(NodeId, u64), HashSet<Hash>>,
    disseminated_blks: HashMap<(NodeId, u64), CoA>,
    pmaker_feedback_send: mpsc::Sender<Vec<u8>>,
    delay: Arc<DelayPool>,
    _notify: Notify,
}

impl NarwhalBlockDissemination {
    pub fn new(
        me: NodeId,
        peer_messenger: Arc<PeerMessenger>,
        threshold_signature: Arc<dyn ThresholdSignature>,
        pmaker_feedback_send: mpsc::Sender<Vec<u8>>,
        delay: Arc<DelayPool>,
    ) -> Self {
        Self {
            me,
            blk_pool: HashMap::new(),
            peer_messenger,
            threshold_signature,
            signatures: HashMap::new(),
            ready_tails: VecDeque::new(),
            conflict_set: HashMap::new(),
            disseminated_blks: HashMap::new(),
            pmaker_feedback_send,
            delay,
            _notify: Notify::new(),
        }
    }

    async fn record_vote(
        &mut self,
        sender: NodeId,
        round: u64,
        digest: Hash,
        voter: NodeId,
        signature: SignPart,
    ) -> Result<(), CopycatError> {
        // do nothing if a conflicting block has already finished dissemination
        if self.disseminated_blks.contains_key(&(sender, round)) {
            return Ok(());
        }

        let blk_key = (sender, round, digest);
        let serialized = bincode::serialize(&blk_key)?;

        let conflict_blks = self
            .conflict_set
            .entry((sender, round))
            .or_insert(HashSet::new());
        conflict_blks.insert(digest);

        let signatures = self.signatures.entry(blk_key).or_insert(HashMap::new());
        signatures.insert(voter, signature);

        let (comb, dur) = self
            .threshold_signature
            .aggregate(&serialized, signatures)?;
        self.delay.process_illusion(dur).await;

        let certificate = match comb {
            Some(signcomb) => CoA {
                sender,
                round,
                digest,
                signature: signcomb,
            },
            None => return Ok(()),
        };

        // got CoA - send to decide stage and send it to pmaker
        self.ready_tails.push_back((sender, round, digest));
        self.pmaker_feedback_send
            .send(bincode::serialize(&certificate)?)
            .await?;

        // got CoA - conflicting blocks will not be disseminated
        self.disseminated_blks.insert((sender, round), certificate);
        let conflict_set = self.conflict_set.remove(&(sender, round)).unwrap();
        for digest in conflict_set {
            self.signatures.remove(&(sender, round, digest));
        }

        Ok(())
    }
}

#[async_trait]
impl BlockDissemination for NarwhalBlockDissemination {
    async fn disseminate(
        &mut self,
        src: NodeId,
        blocks: Vec<(Arc<Block>, Arc<BlkCtx>)>,
    ) -> Result<(), CopycatError> {
        // there will be only one block
        let (blk, blk_ctx) = blocks.last().unwrap();

        let (sender, round) = match blk.header {
            BlockHeader::Aptos { sender, round, .. } => (sender, round),
            _ => unreachable!(),
        };

        if self.me == sender {
            // I am the block builder, disseminate to neighbors
            self.peer_messenger
                .broadcast(MsgType::NewBlock { blk: blk.clone() })
                .await?;
        }
        let digest = sha256(&blk.header)?;
        let content = (&sender, &round, &digest);
        let serialized = bincode::serialize(&content)?;
        let (signature, dur) = self.threshold_signature.sign(&serialized)?;
        self.delay.process_illusion(dur).await;

        self.blk_pool.insert(
            (sender, round, digest),
            (src, (blk.clone(), blk_ctx.clone())),
        );
        if blk_ctx.invalid {
            return Ok(());
        }

        if sender == self.me {
            self.record_vote(sender, round, digest, self.me, signature)
                .await?;
        } else {
            let vote_msg = NarwhalMsgs::Vote {
                sender,
                round,
                digest,
                signature,
            };
            self.peer_messenger
                .broadcast(MsgType::BlkDissemMsg {
                    msg: bincode::serialize(&vote_msg)?,
                })
                .await?;
        }

        Ok(())
    }

    async fn wait_disseminated(&self) -> Result<(), CopycatError> {
        if self.ready_tails.is_empty() {
            // will wait forever
            self._notify.notified().await;
        }
        Ok(())
    }

    async fn get_disseminated(
        &mut self,
    ) -> Result<(NodeId, Vec<(Arc<Block>, Arc<BlkCtx>)>), CopycatError> {
        let tail_id = self.ready_tails.pop_front().unwrap();
        if let Some((src, blk)) = self.blk_pool.remove(&tail_id) {
            return Ok((src, vec![blk]));
        }

        let (sender, round, _) = tail_id;
        let header = BlockHeader::Aptos {
            sender,
            round,
            certificates: vec![],
            merkle_root: Hash::zero(),
            signature: vec![],
        };
        let ctx = Arc::new(BlkCtx::from_header_and_txns(&header, vec![])?); // TODO: add CoA
        let block = Arc::new(Block {
            header,
            txns: vec![],
        });
        Ok((sender, vec![(block, ctx)]))
    }

    async fn handle_peer_msg(&mut self, src: NodeId, msg: Vec<u8>) -> Result<(), CopycatError> {
        let msg: NarwhalMsgs = bincode::deserialize(&msg)?;
        match msg {
            NarwhalMsgs::Vote {
                sender,
                round,
                digest,
                signature,
            } => {
                self.record_vote(sender, round, digest, src, signature)
                    .await?
            }
        }
        Ok(())
    }
}
