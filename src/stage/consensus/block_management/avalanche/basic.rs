use super::{BlockManagement, CurBlockState};
use crate::config::AvalancheBasicConfig;
use crate::context::{BlkCtx, TxnCtx};
use crate::peers::PeerMessenger;
use crate::protocol::block::{Block, BlockHeader};
use crate::protocol::crypto::signature::P2PSignature;
use crate::protocol::crypto::{Hash, PrivKey, PubKey};
use crate::protocol::transaction::{AvalancheTxn, Txn};
use crate::protocol::MsgType;
use crate::stage::DelayPool;
use crate::utils::{CopycatError, NodeId};
use crate::SignatureScheme;

use async_trait::async_trait;
use mailbox_client::SizedMsg;

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use tokio::sync::Notify;
use tokio::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

struct DagNode {
    pub num_parents: usize,
    pub children: HashSet<Hash>,
}

#[derive(Serialize, Deserialize)]
struct PeerReq {
    pub proposer: NodeId,
    pub blk_id: u64,
    pub depth: usize,
    pub dep_set: Vec<Hash>,
}

pub struct AvalancheBlockManagement {
    id: NodeId,
    blk_size: usize,
    txn_pool: HashMap<Hash, (Arc<Txn>, Arc<TxnCtx>)>,
    txn_dag: HashMap<Hash, DagNode>,
    dag_frontier: VecDeque<Hash>,
    // fields for constructing new batch of txns
    blk_counter: u64,
    curr_batch: Vec<Hash>,
    curr_batch_size: usize,
    next_propose_time: Instant,
    proposal_timeout: Duration,
    _notify: Notify,
    // for requesting missing txns
    peer_messenger: Arc<PeerMessenger>,
    pending_blks: HashMap<(NodeId, u64, usize), (Vec<Arc<Txn>>, Vec<Arc<TxnCtx>>, Vec<usize>)>,
    _delay: Arc<DelayPool>,
    _signature_scheme: SignatureScheme,
    _peer_pks: HashMap<NodeId, PubKey>,
    _sk: PrivKey,
    // for control loop
    blk_quota: usize,
    // for reporting data and debugging
    blk_quota_recved: usize,
    blk_fetch_requests_sent: usize,
    txns_requested: usize,
}

impl AvalancheBlockManagement {
    pub fn new(
        id: NodeId,
        p2p_signature: P2PSignature,
        config: AvalancheBasicConfig,
        peer_messenger: Arc<PeerMessenger>,
        delay: Arc<DelayPool>,
    ) -> Self {
        let proposal_timeout = Duration::from_secs_f64(config.proposal_timeout_secs);

        let (signature_scheme, peer_pks, sk) = p2p_signature;

        Self {
            id,
            blk_size: config.blk_size,
            txn_pool: HashMap::new(),
            txn_dag: HashMap::new(),
            dag_frontier: VecDeque::new(),
            blk_counter: 0,
            curr_batch: vec![],
            curr_batch_size: 0,
            next_propose_time: Instant::now() + proposal_timeout,
            proposal_timeout,
            _notify: Notify::new(),
            peer_messenger,
            pending_blks: HashMap::new(),
            _signature_scheme: signature_scheme,
            _peer_pks: peer_pks,
            _sk: sk,
            blk_quota: config.max_inflight_blk,
            blk_quota_recved: config.max_inflight_blk,
            blk_fetch_requests_sent: 0,
            txns_requested: 0,
            _delay: delay,
        }
    }
}

impl AvalancheBlockManagement {
    fn validate_txn(&self, txn: &AvalancheTxn) -> Result<Result<bool, Vec<Hash>>, CopycatError> {
        match txn {
            AvalancheTxn::Send {
                sender: txn_sender,
                in_utxo: txn_in_utxo,
                out_utxo: txn_out_utxo,
                remainder: txn_remainder,
                ..
            } => {
                let mut in_utxo = vec![];
                let mut missing_deps = vec![];
                for in_utxo_hash in txn_in_utxo.iter() {
                    match self.txn_pool.get(in_utxo_hash) {
                        Some((txn, _)) => match txn.as_ref() {
                            Txn::Avalanche { txn } => in_utxo.push(txn),
                            _ => unreachable!(),
                        },
                        None => missing_deps.push(in_utxo_hash.clone()),
                    };
                }

                if missing_deps.len() > 0 {
                    // we cannot reject the txn since the source utxo may not been received yet
                    return Ok(Err(missing_deps));
                }

                let mut input_value = 0;
                for utxo in in_utxo.into_iter() {
                    // add values together to find total input value

                    let value = match utxo {
                        AvalancheTxn::Grant {
                            out_utxo, receiver, ..
                        } => {
                            if receiver == txn_sender {
                                out_utxo
                            } else {
                                return Ok(Ok(false)); // utxo does not belong to sender
                            }
                        }
                        AvalancheTxn::Send {
                            sender,
                            receiver,
                            out_utxo,
                            remainder,
                            ..
                        } => {
                            if receiver == txn_sender {
                                out_utxo
                            } else if sender == txn_sender {
                                remainder
                            } else {
                                return Ok(Ok(false)); // utxo does not belong to sender
                            }
                        }
                        AvalancheTxn::Noop { .. } | AvalancheTxn::PlaceHolder => {
                            unreachable!();
                        }
                    };
                    input_value += value;
                }

                // check if the input values match output values
                if input_value != txn_out_utxo + txn_remainder {
                    return Ok(Ok(false)); // input and output utxo values do not match
                }
            }
            AvalancheTxn::Grant { .. } | AvalancheTxn::Noop { .. } => {
                // grant txns are always correct
                // noops are generated by other nodes to drive consensus and does nothing
            }
            AvalancheTxn::PlaceHolder => {
                unreachable!();
            }
        }

        Ok(Ok(true))
    }
}

#[async_trait]
impl BlockManagement for AvalancheBlockManagement {
    async fn record_new_txn(
        &mut self,
        txn: Arc<Txn>,
        ctx: Arc<TxnCtx>,
    ) -> Result<bool, CopycatError> {
        let txn_hash = ctx.id;
        // ignore duplicates
        if self.txn_pool.contains_key(&txn_hash) {
            return Ok(false);
        }

        let avax_txn = match txn.as_ref() {
            Txn::Avalanche { txn } => txn,
            _ => unreachable!(),
        };

        self.txn_pool
            .insert(txn_hash.clone(), (txn.clone(), ctx.clone()));

        let parents = match avax_txn {
            AvalancheTxn::Grant { .. } => vec![],
            AvalancheTxn::Send { in_utxo, .. } => in_utxo.clone(),
            AvalancheTxn::Noop { .. } | AvalancheTxn::PlaceHolder => unreachable!(), // since Noops are generated by nodes only
        };

        let mut num_parents = 0;
        for parent in parents.iter() {
            if let Some(parent_node) = self.txn_dag.get_mut(parent) {
                parent_node.children.insert(txn_hash);
                num_parents += 1;
            }
        }

        self.txn_dag.insert(
            txn_hash,
            DagNode {
                num_parents: parents.len(),
                children: HashSet::new(),
            },
        );

        if num_parents == 0 {
            self.dag_frontier.push_back(txn_hash);
        }

        Ok(true)
    }

    async fn prepare_new_block(&mut self) -> Result<CurBlockState, CopycatError> {
        let blk_full = loop {
            if self.curr_batch_size >= self.blk_size {
                break CurBlockState::Full;
            }

            let next_txn = match self.dag_frontier.pop_front() {
                Some(txn) => txn,
                None => break CurBlockState::EmptyMempool,
            };

            let node = match self.txn_dag.remove(&next_txn) {
                Some(node) => node,
                None => continue,
            };

            for child in node.children {
                let child_node = match self.txn_dag.get_mut(&child) {
                    Some(node) => node,
                    None => continue,
                };

                // remove node from its parents
                child_node.num_parents -= 1;
                // if all parents removed, add it to frontier
                if child_node.num_parents == 0 {
                    self.dag_frontier.push_back(child);
                }
            }

            let (txn, _) = self.txn_pool.get(&next_txn).unwrap();
            self.curr_batch_size += txn.size()?;
            self.curr_batch.push(next_txn);
        };

        Ok(blk_full)
    }

    async fn wait_to_propose(&self) -> Result<(), CopycatError> {
        // TODO: add noop txns so that parent txns can get voted on
        if self.blk_quota == 0 || self.curr_batch.len() == 0 {
            // we have nothing to propose, sleep forever
            loop {
                self._notify.notified().await;
            }
        } else if self.curr_batch_size >= self.blk_size {
            // we got a complete block, can propose
            return Ok(());
        } else {
            // we got some txns in block, wait for timeout
            crate::sleep_until(self.next_propose_time).await;
            return Ok(());
        }
    }

    async fn get_new_block(&mut self) -> Result<(Arc<Block>, Arc<BlkCtx>), CopycatError> {
        // TODO: add noop txns to drive consensus as needed
        assert!(self.blk_quota > 0);
        let txn_hashs = self.curr_batch.drain(0..);
        self.curr_batch_size = 0;
        // let mut txn_hashs = vec![];
        // std::mem::swap(&mut txn_hashs, &mut self.curr_batch);
        let txns_with_ctx = txn_hashs.map(|txn_hash| self.txn_pool.get(&txn_hash).unwrap().clone());
        let (txns, txn_ctx): (Vec<_>, Vec<_>) = txns_with_ctx.unzip();

        let header = BlockHeader::Avalanche {
            proposer: self.id,
            id: self.blk_counter,
            depth: 0,
        };
        let blk_ctx = BlkCtx::from_header_and_txns(&header, txn_ctx)?;
        pf_trace!(self.id; "sending block query {:?} ({} txns)", header, txns.len());
        let blk = Arc::new(Block { header, txns });

        self.blk_counter += 1;
        self.next_propose_time = Instant::now() + self.proposal_timeout;
        self.blk_quota -= 1;
        Ok((blk, Arc::new(blk_ctx)))
    }

    async fn validate_block(
        &mut self,
        _src: NodeId,
        block: Arc<Block>,
        blk_ctx: Arc<BlkCtx>,
    ) -> Result<Vec<(Arc<Block>, Arc<BlkCtx>)>, CopycatError> {
        pf_trace!(self.id; "Validating block {:?}", block);
        assert!(block.txns.len() == blk_ctx.txn_ctx.len());

        let (proposer, blk_id, depth) = match &block.header {
            BlockHeader::Avalanche {
                proposer,
                id,
                depth,
            } => (*proposer, *id, *depth),
            _ => unreachable!(),
        };

        let mut filtered_txns = vec![];
        let mut blk_txn_ctx = vec![];

        let mut blk_missing_deps = vec![];
        let mut recheck_txn_idx = vec![];

        let mut pending_frontier = vec![];

        for idx in 0..block.txns.len() {
            let txn = &block.txns[idx];
            let txn_ctx = &blk_ctx.txn_ctx[idx];
            let txn_hash = txn_ctx.id;
            if let Some((_, txn_ctx)) = self.txn_pool.get(&txn_hash) {
                // txn has been validated before
                filtered_txns.push(txn.clone());
                blk_txn_ctx.push(txn_ctx.clone());
                if let Some(dag_node) = self.txn_dag.get(&txn_hash) {
                    if dag_node.num_parents == 0 {
                        pending_frontier.push(txn_hash);
                    }
                }
                continue;
            }

            let avax_txn = match txn.as_ref() {
                Txn::Avalanche { txn } => txn,
                _ => unreachable!(),
            };

            // I have not seen this txn before, adding to txn_pool and txn_dag so that it will get proposed later
            let (txn, txn_ctx) = match avax_txn {
                AvalancheTxn::Noop { .. } | AvalancheTxn::PlaceHolder => {
                    // if noop txn, bypass all tests since it is just used to drive consensus
                    // placeholder txns should not be sent across nodes but if this is the case, keep as is
                    // they do not need to be add to txn pool
                    pf_debug!(self.id; "Validating Noop / Placeholder");
                    (txn.clone(), txn_ctx.clone())
                }
                AvalancheTxn::Grant { .. } => {
                    self.txn_pool
                        .insert(txn_hash.clone(), (txn.clone(), txn_ctx.clone()));
                    self.txn_dag.insert(
                        txn_hash,
                        DagNode {
                            num_parents: 0,
                            children: HashSet::new(),
                        },
                    );
                    pending_frontier.push(txn_hash);
                    (txn.clone(), txn_ctx.clone())
                }
                AvalancheTxn::Send { in_utxo, .. } => {
                    // verify validity
                    match self.validate_txn(avax_txn)? {
                        Ok(is_valid) => {
                            if is_valid {
                                // add to txn pool if valid
                                self.txn_pool
                                    .insert(txn_hash.clone(), (txn.clone(), txn_ctx.clone()));
                                let mut num_parents = 0;
                                for parent in in_utxo.iter() {
                                    if let Some(siblings) = self.txn_dag.get_mut(parent) {
                                        siblings.children.insert(txn_hash);
                                        num_parents += 1;
                                    }
                                }
                                self.txn_dag.insert(
                                    txn_hash,
                                    DagNode {
                                        num_parents,
                                        children: HashSet::new(),
                                    },
                                );
                                if num_parents == 0 {
                                    pending_frontier.push(txn_hash);
                                }
                                (txn.clone(), txn_ctx.clone())
                            } else {
                                // otherwise put a place holder
                                let txn = Txn::Avalanche {
                                    txn: AvalancheTxn::PlaceHolder,
                                };
                                let txn_ctx = TxnCtx::from_txn(&txn)?;
                                (Arc::new(txn), Arc::new(txn_ctx))
                            }
                        }
                        Err(txn_missing_deps) => {
                            blk_missing_deps.extend(txn_missing_deps);
                            recheck_txn_idx.push(idx);
                            (txn.clone(), txn_ctx.clone())
                        }
                    }
                }
            };
            filtered_txns.push(txn);
            blk_txn_ctx.push(txn_ctx);
        }

        assert!(filtered_txns.len() == block.txns.len());

        // missing some dependencies, so handle this request when receiving dependencies from peers
        if blk_missing_deps.len() > 0 {
            pf_debug!(self.id; "querying proposer {} for missing txns at block {} depth {}: {:?}", proposer, blk_id, depth + 1, blk_missing_deps);
            while let Some(txn) = pending_frontier.pop() {
                self.dag_frontier.push_front(txn);
            }

            self.blk_fetch_requests_sent += 1;
            self.txns_requested += blk_missing_deps.len();

            let peer_req = PeerReq {
                proposer,
                blk_id,
                depth,
                dep_set: blk_missing_deps,
            };
            let msg = bincode::serialize(&peer_req)?;
            self.peer_messenger
                .send(proposer, Box::new(MsgType::BlockReq { msg }))
                .await?;
            // record the current results
            self.pending_blks.insert(
                (proposer, blk_id, depth),
                (filtered_txns, blk_txn_ctx, recheck_txn_idx),
            );

            return Ok(vec![]);
        }

        // if depth > 0, recursively check the blocks with smaller depth until we reach depth = 0
        let mut curr_depth = depth;
        while curr_depth > 0 {
            curr_depth -= 1;
            let (mut curr_txns, mut curr_txn_ctx, recheck_idx) =
                match self.pending_blks.remove(&(proposer, blk_id, curr_depth)) {
                    Some(record) => record,
                    None => {
                        pf_error!(self.id; "Unexpected block that I did not request");
                        return Ok(vec![]);
                    }
                };

            // validate txns that did not get validated due to missing dependencies
            for idx in recheck_idx.into_iter() {
                let txn = &curr_txns[idx];
                let txn_ctx = &curr_txn_ctx[idx];
                let txn_hash = txn_ctx.id;

                if self.txn_pool.contains_key(&txn_hash) {
                    // txn has been validated before
                    continue;
                }

                let avax_txn = match txn.as_ref() {
                    Txn::Avalanche { txn } => txn,
                    _ => unreachable!(),
                };

                if let AvalancheTxn::Send { in_utxo, .. } = avax_txn {
                    // since we have already queried the peers
                    let valid = self.validate_txn(avax_txn)?.unwrap_or(false);
                    if !valid {
                        pf_warn!(self.id; "got invalid txn in batch {} from proposer {}", blk_id, proposer);
                        // replace with placeholder
                        let txn = Txn::Avalanche {
                            txn: AvalancheTxn::PlaceHolder,
                        };
                        let txn_ctx = TxnCtx::from_txn(&txn)?;
                        curr_txns[idx] = Arc::new(txn);
                        curr_txn_ctx[idx] = Arc::new(txn_ctx);
                    } else {
                        // add to txn_pool
                        self.txn_pool
                            .insert(txn_hash.clone(), (txn.clone(), txn_ctx.clone()));
                        let mut num_parents = 0;
                        for parent in in_utxo.iter() {
                            if let Some(siblings) = self.txn_dag.get_mut(parent) {
                                siblings.children.insert(txn_hash);
                                num_parents += 1;
                            }
                        }
                        self.txn_dag.insert(
                            txn_hash,
                            DagNode {
                                num_parents,
                                children: HashSet::new(),
                            },
                        );
                        if num_parents == 0 {
                            pending_frontier.push(txn_hash);
                        }
                    }
                }
            }

            // we are at the block peer actually asked for voting
            if curr_depth == 0 {
                filtered_txns = curr_txns;
                blk_txn_ctx = curr_txn_ctx;
            }
        }

        // so that txns from peers will be voted early
        while let Some(txn) = pending_frontier.pop() {
            self.dag_frontier.push_front(txn);
        }

        let blk = Arc::new(Block {
            header: BlockHeader::Avalanche {
                proposer,
                id: blk_id,
                depth: curr_depth,
            },
            txns: filtered_txns,
        });
        pf_trace!(self.id; "Emitting validated block {:?}", blk);
        let blk_ctx = Arc::new(BlkCtx::from_header_and_txns(&block.header, blk_txn_ctx)?);
        Ok(vec![(blk, blk_ctx)])
    }

    async fn handle_pmaker_msg(&mut self, _msg: Vec<u8>) -> Result<(), CopycatError> {
        self.blk_quota += 1;
        self.blk_quota_recved += 1;
        Ok(())
    }

    async fn handle_peer_blk_req(
        &mut self,
        peer: NodeId,
        msg: Vec<u8>,
    ) -> Result<(), CopycatError> {
        let peer_req: PeerReq = bincode::deserialize(&msg)?;
        let PeerReq {
            proposer,
            blk_id,
            depth,
            dep_set,
        } = peer_req;

        let txns: Vec<Arc<Txn>> = dep_set
            .into_iter()
            .map(|hash| self.txn_pool.get(&hash))
            .filter(|res| {
                if res.is_none() {
                    pf_error!(self.id; "Querying for txns that I am not aware of, ignoring...");
                }
                res.is_some()
            })
            .map(|res| res.unwrap().0.clone())
            .collect();

        let blk_header = BlockHeader::Avalanche {
            proposer,
            id: blk_id,
            depth: depth + 1,
        };

        let blk = Arc::new(Block {
            header: blk_header,
            txns,
        });

        self.peer_messenger
            .send(peer, Box::new(MsgType::NewBlock { blk }))
            .await?;

        Ok(())
    }

    fn report(&mut self) {
        pf_info!(self.id; "blk_quota_recved: {}, blk_quota: {}", self.blk_quota_recved, self.blk_quota);
        pf_info!(self.id; "blk_fetch_requests_sent: {}, txns_requested: {}", self.blk_fetch_requests_sent, self.txns_requested);
        self.blk_quota_recved = 0;
        self.blk_fetch_requests_sent = 0;
        self.txns_requested = 0;
    }
}
