use super::Decision;
use crate::config::AvalancheConfig;
use crate::peers::PeerMessenger;
use crate::protocol::block::{Block, BlockHeader};
use crate::protocol::crypto::{sha256, Hash, PrivKey, PubKey, Signature};
use crate::protocol::MsgType;
use crate::transaction::{AvalancheTxn, Txn};
use crate::utils::CopycatError;
use crate::{CryptoScheme, NodeId};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::Notify;
use tokio::time::{Duration, Instant};

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

struct ConflictSet {
    pub has_conflicts: bool,
    pub pref: Hash,
    pub count: u64,
}

#[derive(Serialize, Deserialize)]
struct VoteMsg {
    pub round: (NodeId, u64),
    pub votes: Vec<bool>,
    pub signature: Signature,
}

// struct DagNode {
//     pub parents: Vec<Hash>,
//     pub children: Vec<Hash>,
// }

pub struct AvalancheDecision {
    id: NodeId,
    crypto_scheme: CryptoScheme,
    vote_thresh: usize,
    beta1: u64,
    beta2: u64,
    txn_pool: HashMap<Hash, Arc<Txn>>,
    // txn_dag: HashMap<Hash, DagNode>,
    txn_dag: HashMap<Hash, Vec<Hash>>,
    conflict_sets: HashMap<(Hash, PubKey), ConflictSet>, // the cnt of grant txns is always its confidence
    confidence: HashMap<Hash, (bool, u64)>,              // (has_chit, confidence)
    // for voting
    query_pool: HashMap<(NodeId, u64), Vec<Option<Hash>>>,
    votes: HashMap<(NodeId, u64), (usize, Vec<usize>)>,
    preference_cache: HashMap<Hash, bool>,
    // blocks ready to be committed
    commit_queue: Vec<Hash>,
    commit_count: u64,
    batch_emission_time: Instant,
    batch_emission_timeout: Duration,
    _notify: Notify,
    // for p2p comm
    peer_messenger: Arc<PeerMessenger>,
    neighbor_pks: HashMap<NodeId, PubKey>,
    sk: PrivKey,
}

impl AvalancheDecision {
    pub fn new(
        id: NodeId,
        crypto_scheme: CryptoScheme,
        config: AvalancheConfig,
        peer_messenger: Arc<PeerMessenger>,
    ) -> Self {
        // pk can be generated deterministically on demand
        let (_, sk) = crypto_scheme.gen_key_pair(id.into());
        let batch_emission_timeout = Duration::from_secs_f64(config.commit_timeout_secs);
        let vote_thresh = (config.k as f64 * config.alpha).ceil() as usize;
        pf_info!(id; "vote threshold is {}", vote_thresh);

        Self {
            id,
            crypto_scheme,
            vote_thresh,
            beta1: config.beta1,
            beta2: config.beta2,
            txn_pool: HashMap::new(),
            txn_dag: HashMap::new(),
            conflict_sets: HashMap::new(),
            confidence: HashMap::new(),
            query_pool: HashMap::new(),
            votes: HashMap::new(),
            preference_cache: HashMap::new(),
            commit_queue: vec![],
            commit_count: 0,
            batch_emission_time: Instant::now() + batch_emission_timeout,
            batch_emission_timeout,
            _notify: Notify::new(),
            peer_messenger,
            neighbor_pks: HashMap::new(),
            sk,
        }
    }

    fn is_preferred(&self, txn_hash: &Hash) -> bool {
        let txn = match self.txn_pool.get(txn_hash) {
            Some(txn) => txn,
            None => return false,
        };

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

    // fn is_strongly_preferred(&mut self, txn_hash: &Hash) -> bool {
    //     // first check if the txn is already accpeted
    //     let parents = match self.txn_dag.get(txn_hash) {
    //         Some(node) => node.parents.clone(),
    //         None => return true, // since the txn is already accepted
    //     };

    //     match self.preference_cache.get(&txn_hash) {
    //         Some(res) => *res,
    //         None => {
    //             // check if the txn is preferred
    //             if !self.is_preferred(txn_hash) {
    //                 self.preference_cache.insert(txn_hash.clone(), false);
    //                 return false;
    //             }

    //             // recursively check if all its parents are strongly preferred
    //             for parent in parents {
    //                 if !self.is_strongly_preferred(&parent) {
    //                     self.preference_cache.insert(txn_hash.clone(), false);
    //                     return false;
    //                 }
    //             }

    //             self.preference_cache.insert(txn_hash.clone(), true);
    //             true
    //         }
    //     }
    // }

    fn is_strongly_preferred(&mut self, txn_hash: &Hash) -> bool {
        let mut frontier = VecDeque::new();
        frontier.push_back(txn_hash);

        while let Some(hash) = frontier.pop_front() {
            // first check if the txn is already accpeted
            let parents = match self.txn_dag.get(hash) {
                Some(parents) => parents,
                None => return true, // since the txn is already accepted
            };

            let (is_preferred, cache_hit) = match self.preference_cache.get(hash) {
                Some(val) => (*val, true),
                None => (self.is_preferred(hash), false),
            };

            if !cache_hit {
                self.preference_cache.insert(hash.clone(), is_preferred);
            }

            // check if the txn is preferred
            if !is_preferred {
                return false;
            }

            frontier.extend(parents);
        }

        true
    }

    fn handle_votes(&mut self, blk_id: (NodeId, u64), src: NodeId, votes: Vec<bool>) {
        pf_trace!(self.id; "getting vote for block query {:?} from {}", blk_id, src);
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

        // check if good to accept
        let txns = match self.query_pool.get(&blk_id) {
            Some(blk) => blk,
            None => return, // have not got the block query yet, will do the check when it arrives
        };

        assert!(txns.len() == accept_votes.len());
        for idx in 0..txns.len() {
            if accept_votes[idx] == self.vote_thresh {
                // we gathered enough votes
                let txn_hash = match txns[idx] {
                    Some(hash) => hash,
                    None => continue, // the txn is not valid
                };
                pf_trace!(self.id; "looking at txn {} at block idx {}", txn_hash, idx);
                let (has_chit, _) = self.confidence.get_mut(&txn_hash).unwrap();
                *has_chit = true;
                // update the confidence and conflict set for all parents
                let mut dag_frontier = VecDeque::new();
                let mut dedup = HashSet::new();
                dag_frontier.push_back(txn_hash);
                while let Some(next_txn) = dag_frontier.pop_front() {
                    dedup.insert(next_txn);
                    let parents = match self.txn_dag.get(&next_txn) {
                        Some(parents) => parents,
                        None => continue, // the txn is already committed
                    };
                    dag_frontier.extend(parents.iter().filter(|hash| !dedup.contains(hash)));
                    // update confidence
                    let (chit, confidence) = self.confidence.get_mut(&next_txn).unwrap();
                    *confidence += 1;
                    let confidence_score = if *chit { *confidence } else { 0 };
                    // update conflict set
                    let avax_txn = match self.txn_pool.get(&next_txn).unwrap().as_ref() {
                        Txn::Avalanche { txn } => txn,
                        _ => unreachable!(),
                    };

                    let (has_conflicts, count) = match avax_txn {
                        AvalancheTxn::Grant { .. } | AvalancheTxn::Noop { .. } => {
                            (false, confidence_score)
                        }
                        AvalancheTxn::Send {
                            sender, in_utxo, ..
                        } => {
                            let mut count = confidence_score;
                            let mut has_conflicts = false;
                            for input_txn in in_utxo {
                                let utxo = (input_txn.clone(), sender.clone());
                                let ct = self.conflict_sets.get_mut(&utxo).unwrap();
                                if ct.pref == next_txn {
                                    ct.count += 1;
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
                                        // invalidate preference cache for ct.pref, next_txn and their offsprings
                                        // let mut frontier = VecDeque::new();
                                        // frontier.extend(vec![&ct.pref, &next_txn]);
                                        // while let Some(txn) = frontier.pop_front() {
                                        //     self.preference_cache.remove(txn);
                                        //     let children = match self.txn_dag.get(txn) {
                                        //         Some(node) => &node.children,
                                        //         None => continue,
                                        //     };
                                        //     frontier.extend(children);
                                        // }
                                        self.preference_cache.remove(&next_txn);
                                        self.preference_cache.remove(&ct.pref);
                                        // update conflict set
                                        ct.pref = next_txn;
                                        ct.count = 1;
                                    } else {
                                        ct.count += 1;
                                    }
                                }
                                if ct.count < count {
                                    count = ct.count;
                                }
                                if ct.has_conflicts {
                                    has_conflicts = true;
                                }
                            }
                            (has_conflicts, count)
                        }
                        AvalancheTxn::PlaceHolder => unreachable!(),
                    };

                    // add accepted txns to commit queue
                    let mut parents_accepted = true;
                    for parent in parents {
                        if self.txn_dag.contains_key(parent) {
                            parents_accepted = false;
                            break;
                        }
                    }

                    let can_be_accepted = if !has_conflicts {
                        count > self.beta1 // safe early commitment
                    } else {
                        count > self.beta2 // consecutive counter
                    };

                    pf_trace!(self.id; "txn {} - parents accepted: {}, can_be_accepted: {}, count: {}, have conflicts: {}, pending txns: {}", next_txn, parents_accepted, can_be_accepted, count, has_conflicts, self.txn_dag.len());

                    if parents_accepted && can_be_accepted {
                        pf_trace!(self.id; "txn {} accepted", next_txn);
                        self.txn_dag.remove(&next_txn);
                        if matches!(
                            avax_txn,
                            AvalancheTxn::Grant { .. } | AvalancheTxn::Send { .. }
                        ) {
                            self.commit_queue.push(next_txn);
                        }
                    }
                }
            }
            // TODO: if not enough votes, reset counts to 0
        }
    }
}

#[async_trait]
impl Decision for AvalancheDecision {
    async fn new_tail(
        &mut self,
        src: NodeId,
        mut new_tail: Vec<Arc<Block>>,
    ) -> Result<(), CopycatError> {
        let new_blk = if new_tail.len() < 1 {
            return Ok(());
        } else if new_tail.len() == 1 {
            new_tail.remove(0)
        } else {
            unreachable!("Avalanche blocks are in DAG not chain")
        };

        pf_trace!(self.id; "getting new batch of txns {:?}", new_blk.header);

        let (proposer, blk_id) = match new_blk.header {
            BlockHeader::Avalanche { proposer, id } => (proposer, id),
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
        for txn in &new_blk.txns {
            let avax_txn = match txn.as_ref() {
                Txn::Avalanche { txn } => txn,
                _ => unreachable!(),
            };

            match avax_txn {
                AvalancheTxn::Grant { .. } => {
                    // record the txn if never seen it before
                    let txn_hash = sha256(&bincode::serialize(txn)?)?;
                    if !self.txn_pool.contains_key(&txn_hash) {
                        self.txn_pool.insert(txn_hash.clone(), txn.clone());
                        self.txn_dag.insert(txn_hash, vec![]);
                        // self.txn_dag.insert(
                        //     txn_hash,
                        //     DagNode {
                        //         parents: vec![],
                        //         children: vec![],
                        //     },
                        // );
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
                    let txn_hash = sha256(&bincode::serialize(txn)?)?;
                    if !self.txn_pool.contains_key(&txn_hash) {
                        self.txn_pool.insert(txn_hash.clone(), txn.clone());
                        self.txn_dag.insert(txn_hash, in_utxo.clone());
                        // self.txn_dag.insert(
                        //     txn_hash.clone(),
                        //     DagNode {
                        //         parents: in_utxo.clone(),
                        //         children: vec![],
                        //     },
                        // );
                        // for input_txn in in_utxo {
                        //     if let Some(node) = self.txn_dag.get_mut(input_txn) {
                        //         node.children.push(txn_hash.clone());
                        //     }
                        // }
                        self.confidence.insert(txn_hash, (false, 0));
                        // update conflict set as appropriate
                        for unspent_txn in in_utxo {
                            let utxo = (unspent_txn.clone(), sender.clone());
                            match self.conflict_sets.entry(utxo) {
                                Entry::Vacant(e) => {
                                    e.insert(ConflictSet {
                                        has_conflicts: false,
                                        pref: txn_hash.clone(),
                                        count: 0,
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
                    let txn_hash = sha256(&bincode::serialize(txn)?)?;
                    if !self.txn_pool.contains_key(&txn_hash) {
                        self.txn_pool.insert(txn_hash.clone(), txn.clone());
                        self.txn_dag.insert(txn_hash, parents.clone());
                        // self.txn_dag.insert(
                        //     txn_hash,
                        //     DagNode {
                        //         parents: parents.clone(),
                        //         children: vec![],
                        //     },
                        // );
                        // for input_txn in parents {
                        //     if let Some(node) = self.txn_dag.get_mut(input_txn) {
                        //         node.children.push(txn_hash.clone());
                        //     }
                        // }
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
            self.handle_votes((proposer, blk_id), self.id, votes);
        } else {
            // otherwise, send votes to peer that queries the block
            let msg_content = ((proposer, blk_id), votes);
            let serialized_msg = &bincode::serialize(&msg_content)?;
            let signature = self.crypto_scheme.sign(&self.sk, serialized_msg).await?;
            let vote_msg = VoteMsg {
                round: msg_content.0,
                votes: msg_content.1,
                signature,
            };
            self.peer_messenger
                .send(
                    proposer,
                    MsgType::ConsensusMsg {
                        msg: bincode::serialize(&vote_msg)?,
                    },
                )
                .await?;
        }

        Ok(())
    }

    async fn commit_ready(&self) -> Result<(), CopycatError> {
        if self.commit_queue.len() == 0 {
            self._notify.notified().await; // sleep forever
            pf_error!(self.id; "decide stage waking up unexpectedly");
        } else {
            tokio::time::sleep_until(self.batch_emission_time).await;
        }
        Ok(())
    }

    async fn next_to_commit(&mut self) -> Result<(u64, Vec<Arc<Txn>>), CopycatError> {
        let txns: Vec<Arc<Txn>> = self
            .commit_queue
            .drain(0..)
            .map(|hash| self.txn_pool.get(&hash).unwrap().clone())
            .collect();
        pf_debug!(self.id; "committing {} txns", txns.len());
        self.commit_count += 1;
        self.batch_emission_time = Instant::now() + self.batch_emission_timeout;
        Ok((self.commit_count, txns))
    }

    async fn handle_peer_msg(
        &mut self,
        src: NodeId,
        content: Arc<Vec<u8>>,
    ) -> Result<(), CopycatError> {
        let peer_pk = self.neighbor_pks.entry(src).or_insert_with(|| {
            let (pk, _) = self.crypto_scheme.gen_key_pair(src.into());
            pk
        });

        let msg: VoteMsg = bincode::deserialize(content.as_ref())?;
        let content = (msg.round, msg.votes);
        let serialized_content = bincode::serialize(&content)?;
        let (blk_id, votes) = content;
        if self
            .crypto_scheme
            .verify(&peer_pk, &serialized_content, &msg.signature)
            .await?
        {
            self.handle_votes(blk_id, src, votes)
        }

        Ok(())
    }
}
