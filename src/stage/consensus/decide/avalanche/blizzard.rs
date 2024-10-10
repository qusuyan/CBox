use super::{Decision, VoteMsg};
use crate::config::AvalancheCorrectConfig;
use crate::context::{BlkCtx, TxnCtx};
use crate::peers::PeerMessenger;
use crate::protocol::block::{Block, BlockHeader};
use crate::protocol::crypto::{Hash, PrivKey, PubKey};
use crate::protocol::MsgType;
use crate::transaction::{AvalancheTxn, Txn};
use crate::{CopycatError, CryptoScheme, NodeId};

use async_trait::async_trait;

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::sync::Notify;

use atomic_float::AtomicF64;

struct ConflictSet {
    pub has_conflicts: bool,
    pub pref: Hash,
    pub total_rounds: u64, // equal to the sum of the confidence of all txns in the set
}

struct DagNode {
    pub parents: Vec<Hash>,
    pub children: Vec<Hash>,
}

pub struct BlizzardDecision {
    id: NodeId,
    crypto_scheme: CryptoScheme,
    k: usize,
    vote_thresh: usize,
    beta1: u64,
    beta2: u64,
    txn_pool: HashMap<Hash, (Arc<Txn>, Arc<TxnCtx>)>,
    txn_dag: HashMap<Hash, DagNode>,
    conflict_sets: HashMap<(Hash, PubKey), ConflictSet>,
    confidence: HashMap<Hash, (bool, u64)>, // (has_chit, confidence)
    // for voting
    query_pool: HashMap<(NodeId, u64), Vec<Option<Hash>>>, // TODO: add timeouts
    finished_query: HashSet<(NodeId, u64)>,
    votes: HashMap<(NodeId, u64), (usize, Vec<usize>)>,
    preference_cache: HashMap<Hash, bool>,
    // blocks ready to be committed
    commit_queue: VecDeque<(u64, Vec<Hash>)>,
    _notify: Notify,
    // for p2p comm
    peer_messenger: Arc<PeerMessenger>,
    neighbor_pks: HashMap<NodeId, PubKey>,
    sk: PrivKey,
    // control loop
    pmaker_feedback_send: mpsc::Sender<Vec<u8>>,
    // statistics for debugging
    is_strongly_preferred_calls: usize,
    is_preferred_checks: usize,
    // batch sleep time to reduce overhead
    delay: Arc<AtomicF64>,
}

impl BlizzardDecision {
    pub fn new(
        id: NodeId,
        crypto_scheme: CryptoScheme,
        config: AvalancheCorrectConfig,
        peer_messenger: Arc<PeerMessenger>,
        pmaker_feedback_send: mpsc::Sender<Vec<u8>>,
        delay: Arc<AtomicF64>,
    ) -> Self {
        // pk can be generated deterministically on demand
        let (_, sk) = crypto_scheme.gen_key_pair(id.into());
        let vote_thresh = (config.k as f64 * config.alpha).ceil() as usize;
        pf_info!(id; "vote threshold is {}", vote_thresh);

        Self {
            id,
            crypto_scheme,
            k: config.k,
            vote_thresh,
            beta1: config.beta1,
            beta2: config.beta2,
            txn_pool: HashMap::new(),
            txn_dag: HashMap::new(),
            conflict_sets: HashMap::new(),
            confidence: HashMap::new(),
            query_pool: HashMap::new(),
            finished_query: HashSet::new(),
            votes: HashMap::new(),
            preference_cache: HashMap::new(),
            commit_queue: VecDeque::new(),
            _notify: Notify::new(),
            peer_messenger,
            neighbor_pks: HashMap::new(),
            sk,
            pmaker_feedback_send,
            is_strongly_preferred_calls: 0,
            is_preferred_checks: 0,
            delay,
        }
    }

    fn is_preferred(&self, txn_hash: &Hash) -> bool {
        let (txn, _) = self.txn_pool.get(txn_hash).unwrap();

        let avax_txn = match txn.as_ref() {
            Txn::Avalanche { txn } => txn,
            _ => unreachable!(),
        };

        match avax_txn {
            AvalancheTxn::Grant { .. } | AvalancheTxn::Noop { .. } => true,
            AvalancheTxn::Send {
                sender, in_utxo, ..
            } => {
                let mut preferred = true;
                for txn_input in in_utxo {
                    let utxo = (txn_input.clone(), sender.clone());
                    let ct = self.conflict_sets.get(&utxo).unwrap();
                    if ct.pref != *txn_hash {
                        preferred = false;
                    }
                }
                preferred
            }
            AvalancheTxn::PlaceHolder => unreachable!(),
        }
    }

    fn is_strongly_preferred(&mut self, txn_hash: &Hash) -> bool {
        self.is_strongly_preferred_calls += 1;

        let mut frontier = VecDeque::new();
        frontier.push_back(txn_hash);
        let mut checked = vec![];

        let result = loop {
            let hash = match frontier.pop_front() {
                Some(hash) => hash,
                None => break true,
            };

            self.is_preferred_checks += 1;
            // first check if the txn is already accpeted
            let dag_node = match self.txn_dag.get(hash) {
                Some(node) => node,
                None => continue, // since the txn is already accepted
            };

            if let Some(strongly_preferred) = self.preference_cache.get(hash) {
                if *strongly_preferred {
                    continue;
                } else {
                    break false;
                }
            }

            // not in cache - recursively perform the check
            checked.push(hash);

            if !self.is_preferred(hash) {
                break false;
            }

            let parents: Vec<&Hash> = dag_node.parents.iter().collect();
            frontier.extend(parents);
        };

        for txn in checked {
            self.preference_cache.insert(*txn, result);
        }

        result
    }

    async fn record_votes(
        &mut self,
        blk_id: (NodeId, u64),
        src: NodeId,
        votes: Vec<bool>,
    ) -> Result<(), CopycatError> {
        // ignore extra votes
        if self.finished_query.contains(&blk_id) {
            return Ok(());
        }

        pf_trace!(self.id; "getting vote for block query {:?} from {}: {:?}", blk_id, src, votes);
        let (votes_received, accept_votes) = self
            .votes
            .entry(blk_id)
            .or_insert((0, vec![0; votes.len()]));
        assert!(accept_votes.len() == votes.len());
        *votes_received += 1;
        for idx in 0..votes.len() {
            if votes[idx] {
                accept_votes[idx] += 1;
            }
        }

        if *votes_received >= self.k {
            // received all votes
            self.handle_votes(blk_id).await?;
        }

        Ok(())
    }

    // TODO: call this function when timeout
    async fn handle_votes(&mut self, blk_id: (NodeId, u64)) -> Result<(), CopycatError> {
        let txns = match self.query_pool.remove(&blk_id) {
            Some(txns) => txns,
            None => return Ok(()), // if the batch has not yet been received or if the batch is already committed
        };
        let mut txns_to_be_committed = vec![];

        let (_, accept_votes) = self.votes.remove(&blk_id).unwrap();
        assert!(txns.len() == accept_votes.len());
        self.finished_query.insert(blk_id);
        for idx in 0..txns.len() {
            let txn_hash = match txns[idx] {
                Some(hash) => hash,
                None => continue, // the txn is not valid
            };
            if accept_votes[idx] >= self.vote_thresh {
                // we gathered enough votes
                pf_trace!(self.id; "looking at txn {} at block idx {}", txn_hash, idx);
                let (has_chit, _) = self.confidence.get_mut(&txn_hash).unwrap();
                *has_chit = true;
                // update the confidence and conflict set for all parents
                let mut dag_frontier = VecDeque::new();
                let mut dedup = HashSet::new();
                dag_frontier.push_back(txn_hash);
                while let Some(next_txn) = dag_frontier.pop_front() {
                    if dedup.contains(&next_txn) {
                        continue;
                    }
                    dedup.insert(next_txn);
                    let dag_node = match self.txn_dag.get(&next_txn) {
                        Some(node) => node,
                        None => continue, // the txn is already committed
                    };
                    dag_frontier.extend(dag_node.parents.iter());
                    // update confidence
                    let (chit, confidence) = self.confidence.get_mut(&next_txn).unwrap();
                    *confidence += 1;
                    let confidence_score = if *chit { *confidence } else { 0 };
                    // update conflict set
                    let avax_txn = match self.txn_pool.get(&next_txn).unwrap().0.as_ref() {
                        Txn::Avalanche { txn } => txn,
                        _ => unreachable!(),
                    };

                    let (has_conflicts, conflict_count) = match avax_txn {
                        AvalancheTxn::Grant { .. } | AvalancheTxn::Noop { .. } => (false, 0),
                        AvalancheTxn::Send {
                            sender, in_utxo, ..
                        } => {
                            let mut conflict_count = 0;
                            let mut has_conflicts = false;
                            for input_txn in in_utxo {
                                let utxo = (input_txn.clone(), sender.clone());
                                let ct = self.conflict_sets.get_mut(&utxo).unwrap();

                                if ct.pref == next_txn {
                                    // do nothing
                                } else {
                                    let pref_confidence_score = {
                                        let (chit, conf) = self.confidence.get(&ct.pref).unwrap();
                                        if *chit {
                                            *conf
                                        } else {
                                            0
                                        }
                                    };
                                    if pref_confidence_score < confidence_score {
                                        // update conflict set
                                        ct.pref = next_txn;
                                        // invalidate preference cache for ct.pref, next_txn and their offsprings
                                        let mut frontier = VecDeque::new();
                                        frontier.extend(vec![&ct.pref, &next_txn]);
                                        while let Some(txn) = frontier.pop_front() {
                                            self.preference_cache.remove(txn);
                                            let children = match self.txn_dag.get(txn) {
                                                Some(node) => &node.children,
                                                None => continue,
                                            };
                                            frontier.extend(children);
                                        }
                                        // self.preference_cache.remove(&next_txn);
                                        // self.preference_cache.remove(&ct.pref);
                                    }
                                }

                                // add one round for this conflict set
                                ct.total_rounds += 1;

                                // txn conflicts with other txns on one of the input utxos
                                if ct.has_conflicts {
                                    has_conflicts = true;
                                }
                                // conflict_count is the total rounds for this conflict set - my_count
                                // account for all conflicting txns on any UTXO
                                conflict_count += ct.total_rounds - confidence_score;
                            }
                            (has_conflicts, conflict_count)
                        }
                        AvalancheTxn::PlaceHolder => unreachable!(),
                    };

                    // add accepted txns to commit queue
                    let mut parents_accepted = true;
                    for parent in dag_node.parents.iter() {
                        if self.txn_dag.contains_key(parent) {
                            parents_accepted = false;
                            break;
                        }
                    }

                    // my_count is always the confidence score of current txn
                    let is_accepted = if parents_accepted && !has_conflicts {
                        confidence_score > self.beta1 + conflict_count // safe early commitment
                    } else {
                        confidence_score > self.beta2 + conflict_count // consecutive counter
                    };

                    pf_trace!(self.id; "txn {} - is_accepted: {}, parents accepted: {}, have conflicts: {}, confidence_score: {}, conflict_count: {}, pending txns: {}", 
                                    next_txn, is_accepted, parents_accepted, has_conflicts, confidence_score, conflict_count, self.txn_dag.len());

                    if is_accepted {
                        pf_trace!(self.id; "txn {} accepted", next_txn);
                        self.txn_dag.remove(&next_txn);
                        self.preference_cache.remove(&next_txn);
                        if matches!(
                            avax_txn,
                            AvalancheTxn::Grant { .. } | AvalancheTxn::Send { .. }
                        ) {
                            txns_to_be_committed.push(next_txn);
                        }
                    }
                }
            }
        }

        self.commit_queue
            .push_back((blk_id.1, txns_to_be_committed));
        // send to pmaker a batch is voted
        self.pmaker_feedback_send.send(vec![]).await?;

        Ok(())
    }
}

#[async_trait]
impl Decision for BlizzardDecision {
    async fn new_tail(
        &mut self,
        src: NodeId,
        mut new_tail: Vec<(Arc<Block>, Arc<BlkCtx>)>,
    ) -> Result<(), CopycatError> {
        let (new_blk, new_blk_ctx) = if new_tail.len() < 1 {
            return Ok(());
        } else if new_tail.len() == 1 {
            new_tail.remove(0)
        } else {
            unreachable!("Avalanche blocks are in DAG not chain")
        };

        pf_trace!(self.id; "getting new batch of txns {:?} - txns: {:?}", new_blk, new_blk.txns);

        let (proposer, blk_id) = match new_blk.header {
            BlockHeader::Avalanche {
                proposer,
                id,
                depth,
            } => {
                assert!(depth == 0);
                (proposer, id)
            }
            _ => unreachable!(),
        };

        assert!(src == proposer);

        // record txns if not yet done
        // enum VoteParams {
        //     Vote { vote: bool },
        //     Check { hash: Hash },
        // }
        let mut votes = vec![];
        // let mut vote_params = vec![];
        let mut query_txns = vec![];

        assert!(new_blk.txns.len() == new_blk_ctx.txn_ctx.len());
        for idx in 0..new_blk.txns.len() {
            let txn = &new_blk.txns[idx];
            let txn_ctx = &new_blk_ctx.txn_ctx[idx];
            let avax_txn = match txn.as_ref() {
                Txn::Avalanche { txn } => txn,
                _ => unreachable!(),
            };

            match avax_txn {
                AvalancheTxn::Grant { .. } => {
                    // record the txn if never seen it before
                    let txn_hash = txn_ctx.id;
                    if !self.txn_pool.contains_key(&txn_hash) {
                        self.txn_pool
                            .insert(txn_hash.clone(), (txn.clone(), txn_ctx.clone()));
                        // self.txn_dag.insert(txn_hash, vec![]);
                        self.txn_dag.insert(
                            txn_hash,
                            DagNode {
                                parents: vec![],
                                children: vec![],
                            },
                        );
                        self.confidence.insert(txn_hash, (false, 0));
                    }
                    // vote true on grant msgs since they cannot conflict with others
                    votes.push(true);
                    // vote_params.push(VoteParams::Vote { vote: true });
                    query_txns.push(Some(txn_hash));
                }
                AvalancheTxn::Send {
                    sender, in_utxo, ..
                } => {
                    // record the txn if never seen it before
                    let txn_hash = txn_ctx.id;
                    if !self.txn_pool.contains_key(&txn_hash) {
                        self.txn_pool
                            .insert(txn_hash.clone(), (txn.clone(), txn_ctx.clone()));
                        // self.txn_dag.insert(txn_hash, in_utxo.clone());
                        self.txn_dag.insert(
                            txn_hash.clone(),
                            DagNode {
                                parents: in_utxo.clone(),
                                children: vec![],
                            },
                        );
                        for input_txn in in_utxo {
                            if let Some(node) = self.txn_dag.get_mut(input_txn) {
                                node.children.push(txn_hash.clone());
                            }
                        }
                        self.confidence.insert(txn_hash, (false, 0));
                        // update conflict set as appropriate
                        for unspent_txn in in_utxo {
                            let utxo = (unspent_txn.clone(), sender.clone());
                            match self.conflict_sets.entry(utxo) {
                                Entry::Vacant(e) => {
                                    e.insert(ConflictSet {
                                        has_conflicts: false,
                                        pref: txn_hash.clone(),
                                        total_rounds: 0,
                                    });
                                }
                                Entry::Occupied(mut e) => e.get_mut().has_conflicts = true,
                            }
                        }
                    }
                    votes.push(self.is_strongly_preferred(&txn_hash));
                    // vote_params.push(VoteParams::Check {
                    //     hash: txn_hash.clone(),
                    // });
                    query_txns.push(Some(txn_hash));
                }
                AvalancheTxn::Noop { parents } => {
                    let txn_hash = txn_ctx.id;
                    if !self.txn_pool.contains_key(&txn_hash) {
                        self.txn_pool
                            .insert(txn_hash.clone(), (txn.clone(), txn_ctx.clone()));
                        // self.txn_dag.insert(txn_hash, parents.clone());
                        self.txn_dag.insert(
                            txn_hash,
                            DagNode {
                                parents: parents.clone(),
                                children: vec![],
                            },
                        );
                        for input_txn in parents {
                            if let Some(node) = self.txn_dag.get_mut(input_txn) {
                                node.children.push(txn_hash.clone());
                            }
                        }
                        self.confidence.insert(txn_hash, (false, 0));
                    }
                    votes.push(self.is_strongly_preferred(&txn_hash));
                    // vote_params.push(VoteParams::Check {
                    //     hash: txn_hash.clone(),
                    // });
                    query_txns.push(Some(txn_hash));
                }
                AvalancheTxn::PlaceHolder => {
                    // no need to record the txn
                    // vote no since the txn is not valid
                    votes.push(false);
                    // vote_params.push(VoteParams::Vote { vote: false });
                    query_txns.push(None);
                }
            }
        }

        // let (_, votes_result) = async_scoped::TokioScope::scope_and_block(|s| {
        //     for param in vote_params.iter() {
        //         match param {
        //             VoteParams::Vote { vote } => s.spawn(async {
        //                 return *vote;
        //             }),
        //             VoteParams::Check { hash } => s.spawn(self.is_strongly_preferred(hash)),
        //         }
        //     }
        // });
        // let votes = votes_result.into_iter().map(|res| res.unwrap()).collect();

        if proposer == self.id {
            // if queried by myself, record block and and handle votes locally
            self.query_pool.insert((proposer, blk_id), query_txns);
            self.record_votes((proposer, blk_id), self.id, votes)
                .await?;
        } else {
            // otherwise, send votes to peer that queries the block
            let msg_content = ((proposer, blk_id), votes);
            let serialized_msg = &bincode::serialize(&msg_content)?;
            let (signature, stime) = self.crypto_scheme.sign(&self.sk, serialized_msg)?;
            let delay = self.delay.fetch_add(stime, Ordering::Relaxed);
            let vote_msg = VoteMsg {
                round: msg_content.0,
                votes: msg_content.1,
                signature,
            };
            self.peer_messenger
                .delayed_send(
                    proposer,
                    MsgType::ConsensusMsg {
                        msg: bincode::serialize(&vote_msg)?,
                    },
                    Duration::from_secs_f64(delay + stime),
                )
                .await?;
        }

        Ok(())
    }

    async fn commit_ready(&self) -> Result<(), CopycatError> {
        if self.commit_queue.len() == 0 {
            self._notify.notified().await; // sleep forever
            pf_error!(self.id; "decide stage waking up unexpectedly");
        }
        Ok(())
    }

    async fn next_to_commit(&mut self) -> Result<(u64, Vec<Arc<Txn>>), CopycatError> {
        let (blk_id, txn_hashes) = self.commit_queue.pop_front().unwrap();
        let txns: Vec<Arc<Txn>> = txn_hashes
            .into_iter()
            .map(|hash| self.txn_pool.get(&hash).unwrap().0.clone())
            .collect();
        pf_debug!(self.id; "committing {} txns", txns.len());
        Ok((blk_id, txns))
    }

    async fn handle_peer_msg(&mut self, src: NodeId, content: Vec<u8>) -> Result<(), CopycatError> {
        let msg: VoteMsg = bincode::deserialize(content.as_ref())?;
        if self.finished_query.contains(&msg.round) {
            return Ok(());
        }

        let peer_pk = self.neighbor_pks.entry(src).or_insert_with(|| {
            let (pk, _) = self.crypto_scheme.gen_key_pair(src.into());
            pk
        });

        let content = (msg.round, msg.votes);
        let serialized_content = bincode::serialize(&content)?;
        let (blk_id, votes) = content;
        let (valid, vtime) =
            self.crypto_scheme
                .verify(&peer_pk, &serialized_content, &msg.signature)?;
        self.delay.fetch_add(vtime, Ordering::Relaxed);
        if valid {
            self.record_votes(blk_id, src, votes).await?;
        }

        Ok(())
    }

    fn report(&mut self) {
        pf_info!(self.id; "In the last minute: is_strongly_preferred_calls: {}, is_preferred_checks: {}", self.is_strongly_preferred_calls, self.is_preferred_checks);
        pf_info!(self.id; "working set size: txn_dag: {}, perference_cache: {}", self.txn_dag.len(), self.preference_cache.len());
        self.is_strongly_preferred_calls = 0;
        self.is_preferred_checks = 0;
    }
}
