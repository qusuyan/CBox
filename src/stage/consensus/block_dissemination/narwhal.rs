use super::BlockDissemination;
use crate::context::{BlkCtx, BlkData};
use crate::peers::PeerMessenger;
use crate::protocol::block::{Block, BlockHeader};
use crate::protocol::crypto::threshold_signature::{SignPart, ThresholdSignature};
use crate::protocol::crypto::{sha256, Hash};
use crate::protocol::types::aptos::CoA;
use crate::protocol::MsgType;
use crate::stage::DelayPool;
use crate::{CopycatError, NodeId};

use async_trait::async_trait;
use primitive_types::U256;
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
    ready_tails: VecDeque<(NodeId, u64, Hash)>,
    // narwhal consensus
    round: u64,
    disseminated_blks: HashMap<(NodeId, u64), CoA>,
    signatures: HashMap<(NodeId, u64, Hash), HashMap<NodeId, SignPart>>,
    conflict_set: HashMap<(NodeId, u64), (HashSet<Hash>, Option<Hash>)>,
    future_blks: HashMap<u64, HashMap<NodeId, Hash>>,
    // P2P
    peer_messenger: Arc<PeerMessenger>,
    threshold_signature: Arc<dyn ThresholdSignature>,
    //
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
            ready_tails: VecDeque::new(),
            round: 0,
            signatures: HashMap::new(),
            conflict_set: HashMap::new(),
            disseminated_blks: HashMap::new(),
            future_blks: HashMap::new(),
            peer_messenger,
            threshold_signature,
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
            if voter == self.me {
                self.ready_tails.push_back((sender, round, digest));
            }
            return Ok(());
        }

        let blk_key = (sender, round, digest);
        let serialized = bincode::serialize(&blk_key)?;

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

        pf_debug!(self.me; "certificate of availability formed for block (sender: {}, round: {}, digest: {})", sender, round, digest);

        // got CoA - send to decide stage and send it to pmaker
        self.ready_tails.push_back((sender, round, digest));
        self.pmaker_feedback_send
            .send(bincode::serialize(&certificate)?)
            .await?;

        // got CoA - conflicting blocks will not be disseminated
        self.disseminated_blks.insert((sender, round), certificate);
        let (conflict_set, _) = self.conflict_set.remove(&(sender, round)).unwrap();
        for conflict_digest in conflict_set {
            self.signatures.remove(&(sender, round, conflict_digest));
            if conflict_digest != digest {
                self.blk_pool.remove(&(sender, round, conflict_digest));
            }
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

        let digest = sha256(&blk.header)?;
        let content = (&sender, &round, &digest);
        let serialized = bincode::serialize(&content)?;
        let (signature, dur) = self.threshold_signature.sign(&serialized)?;
        self.delay.process_illusion(dur).await;

        self.blk_pool.insert(
            (sender, round, digest),
            (src, (blk.clone(), blk_ctx.clone())),
        );

        let (conflict_blks, voted) = self
            .conflict_set
            .entry((sender, round))
            .or_insert((HashSet::new(), None));
        conflict_blks.insert(digest);

        if self.me == sender {
            // I am the block builder, disseminate to neighbors
            self.peer_messenger
                .broadcast(MsgType::NewBlock { blk: blk.clone() })
                .await?;
            if self.round < round {
                self.round = round;
                if let Some(pending) = self.future_blks.remove(&self.round) {
                    for (pending_sender, pending_digest) in pending {
                        pf_debug!(self.me; "voting for block (sender: {}, round: {}, digest: {})", pending_sender, self.round, pending_digest);

                        let content = (&pending_sender, &self.round, &pending_digest);
                        let serialized = bincode::serialize(&content)?;
                        let (pending_signature, dur) =
                            self.threshold_signature.sign(&serialized)?;
                        self.delay.process_illusion(dur).await;

                        let pending_vote_msg = NarwhalMsgs::Vote {
                            sender: pending_sender,
                            round: self.round,
                            digest: pending_digest,
                            signature: pending_signature.clone(),
                        };
                        self.peer_messenger
                            .broadcast(MsgType::BlkDissemMsg {
                                msg: bincode::serialize(&pending_vote_msg)?,
                            })
                            .await?;
                        self.record_vote(
                            pending_sender,
                            self.round,
                            pending_digest,
                            self.me,
                            pending_signature,
                        )
                        .await?;
                    }
                }
            }
        } else {
            // only vote for blocks in the current round
            if round < self.round {
                return Ok(());
            } else if round > self.round {
                let pending = self.future_blks.entry(round).or_insert(HashMap::new());
                if !pending.contains_key(&sender) {
                    // first block seen at this position
                    pending.insert(sender, digest);
                }
                return Ok(());
            }

            // only vote for the first one received from creator of round r
            if voted.is_some() {
                return Ok(());
            }
        }

        pf_debug!(self.me; "voting for block (sender: {}, round: {}, digest: {})", sender, round, digest);

        let vote_msg = NarwhalMsgs::Vote {
            sender,
            round,
            digest,
            signature: signature.clone(),
        };
        self.peer_messenger
            .broadcast(MsgType::BlkDissemMsg {
                msg: bincode::serialize(&vote_msg)?,
            })
            .await?;
        self.record_vote(sender, round, digest, self.me, signature)
            .await?;

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
        let (sender, round, digest) = self.ready_tails.pop_front().unwrap();
        let certificate = self
            .disseminated_blks
            .get(&(sender, round))
            .unwrap()
            .clone(); // TODO: remove clone()

        if let Some((src, (blk, ctx))) = self.blk_pool.get(&(sender, round, digest)) {
            let new_ctx = Arc::new(ctx.with_data(BlkData::Aptos { certificate }));
            return Ok((*src, vec![(blk.clone(), new_ctx)]));
        }

        let header = BlockHeader::Aptos {
            sender,
            round,
            certificates: vec![],
            merkle_root: Hash(U256::zero()),
            signature: vec![],
        };
        let ctx = Arc::new(
            BlkCtx::from_header_and_txns(&header, vec![])?
                .with_data(BlkData::Aptos { certificate }),
        );
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
                let (conflict_blks, _) = self
                    .conflict_set
                    .entry((sender, round))
                    .or_insert((HashSet::new(), None));
                conflict_blks.insert(digest);
                pf_debug!(self.me; "received vote from {} for block (sender: {}, round: {}, digest: {})", src, sender, round, digest);
                self.record_vote(sender, round, digest, src, signature)
                    .await?
            }
        }
        Ok(())
    }
}
