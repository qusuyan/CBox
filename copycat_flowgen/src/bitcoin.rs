use super::{FlowGen, Stats};
use crate::{ClientId, FlowGenId};
use copycat::protocol::crypto::{Hash, PrivKey, PubKey};
use copycat::protocol::transaction::{BitcoinTxn, Txn};
use copycat::{CopycatError, NodeId, SignatureScheme};

use async_trait::async_trait;
use rand::Rng;

use std::collections::hash_map::Entry;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::Notify;
use tokio::time::{Duration, Instant};

const UNSET: usize = 0;
const MAX_BATCH_FREQ: usize = 20;

struct ChainInfo {
    chain_length: u64,
    txn_count: HashMap<u64, u64>,
    total_committed: u64, // includes false commits
}

pub struct BitcoinFlowGen {
    max_inflight: usize,
    batch_frequency: usize,
    batch_size: usize,
    script_size: usize,
    next_batch_time: Instant,
    crypto: SignatureScheme,
    client_list: Vec<ClientId>,
    utxos: HashMap<ClientId, HashMap<PubKey, VecDeque<(Hash, u64)>>>,
    accounts: HashMap<ClientId, Vec<(PubKey, PrivKey)>>,
    in_flight: HashMap<Hash, Instant>,
    chain_info: HashMap<NodeId, ChainInfo>,
    latencies: Vec<f64>,
    _notify: Notify,
}

impl BitcoinFlowGen {
    pub fn new(
        id: FlowGenId,
        client_list: Vec<ClientId>,
        num_accounts: usize,
        script_size: Option<usize>,
        max_inflight: usize,
        frequency: usize,
        crypto: SignatureScheme,
    ) -> Self {
        let accounts_per_node = if client_list.len() == 0 {
            0
        } else {
            num_accounts / client_list.len()
        };

        let mut accounts = HashMap::new();
        let mut utxos = HashMap::new();
        let mut i = 0u64;
        for client in client_list.iter() {
            for _ in 0..accounts_per_node as u64 {
                let seed = (id << 32) | i;
                i += 1;
                let (pubkey, privkey) = crypto.gen_key_pair(seed);
                utxos
                    .entry(*client)
                    .or_insert(HashMap::new())
                    .insert(pubkey.clone(), VecDeque::new());
                accounts
                    .entry(*client)
                    .or_insert(vec![])
                    .push((pubkey, privkey));
            }
        }

        let (batch_size, batch_frequency) = if frequency == UNSET {
            (max_inflight, UNSET)
        } else if frequency < MAX_BATCH_FREQ {
            (1, frequency)
        } else {
            (frequency / MAX_BATCH_FREQ, MAX_BATCH_FREQ)
        };

        Self {
            max_inflight,
            batch_frequency,
            batch_size,
            script_size: script_size.unwrap_or(0),
            next_batch_time: Instant::now(),
            crypto,
            client_list,
            utxos,
            accounts,
            in_flight: HashMap::new(),
            latencies: vec![],
            chain_info: HashMap::new(),
            _notify: Notify::new(),
        }
    }
}

#[async_trait]
impl FlowGen for BitcoinFlowGen {
    async fn setup_txns(&mut self) -> Result<Vec<(ClientId, Arc<Txn>)>, CopycatError> {
        let mut txns = Vec::new();
        for (node, utxo_map) in self.utxos.iter_mut() {
            for (account, utxos) in utxo_map.iter_mut() {
                let txn = Txn::Bitcoin {
                    txn: BitcoinTxn::Grant {
                        out_utxo: 100,
                        receiver: account.clone(),
                    },
                };
                let hash = txn.compute_id()?;
                utxos.push_back((hash, 100));
                txns.push((*node, Arc::new(txn)));
            }
        }

        Ok(txns)
    }

    async fn wait_next(&self) -> Result<(), CopycatError> {
        // do nothing if no client
        if self.client_list.len() == 0 {
            self._notify.notified().await;
        }

        loop {
            if self.max_inflight == UNSET || self.in_flight.len() < self.max_inflight {
                if Instant::now() < self.next_batch_time {
                    tokio::time::sleep_until(self.next_batch_time).await;
                }
                return Ok(());
            }
            tokio::task::yield_now().await;
        }
    }

    async fn next_txn_batch(&mut self) -> Result<Vec<(ClientId, Arc<Txn>)>, CopycatError> {
        let mut batch = vec![];
        let batch_size = if self.max_inflight == UNSET {
            self.batch_size
        } else {
            std::cmp::min(self.batch_size, self.max_inflight - self.in_flight.len())
        };
        for _ in 0..batch_size {
            let node = self.client_list[rand::random::<usize>() % self.client_list.len()];
            let accounts = self.accounts.get(&node).unwrap();
            let utxos = self.utxos.get_mut(&node).unwrap();

            let (sender, remainder, recver, out_utxo, txn) = loop {
                let sender = rand::random::<usize>() % accounts.len();
                let mut receiver = rand::random::<usize>() % (accounts.len() - 1);
                if receiver >= sender {
                    receiver += 1; // avoid sending to self
                }

                let (sender_pk, sender_sk) = accounts.get(sender).unwrap();
                let (recver_pk, _) = accounts.get(receiver).unwrap();
                let sender_utxos = utxos.get_mut(sender_pk).unwrap();
                if sender_utxos.is_empty() {
                    continue; // retry
                }

                let (in_utxo_raw, amount) = sender_utxos.pop_front().unwrap();
                let in_utxo = vec![in_utxo_raw];
                let serialized_in_utxo = bincode::serialize(&in_utxo)?;
                let (sender_signature, _) = self.crypto.sign(sender_sk, &serialized_in_utxo)?;
                let out_utxo = std::cmp::min(amount, 10);
                let remainder = amount - out_utxo;
                let txn = Arc::new(Txn::Bitcoin {
                    txn: BitcoinTxn::Send {
                        sender: sender_pk.clone(),
                        in_utxo,
                        receiver: recver_pk.clone(),
                        out_utxo,
                        remainder,
                        sender_signature,
                        script_bytes: self.script_size,
                        script_runtime_sec: 0f64,
                        script_succeed: true,
                    },
                });
                break (sender_pk, remainder, recver_pk, out_utxo, txn);
            };

            let txn_hash = txn.compute_id()?;
            if remainder > 0 {
                let sender_utxos = utxos.get_mut(sender).unwrap();
                sender_utxos.push_back((txn_hash.clone(), remainder));
            }
            if out_utxo > 0 {
                let recver_utxos = utxos.get_mut(recver).unwrap();
                recver_utxos.push_back((txn_hash.clone(), out_utxo));
            }

            self.in_flight.insert(txn_hash, Instant::now());
            batch.push((node, txn));
        }

        if self.batch_frequency != UNSET {
            // poisson interarrival time
            let interarrival_time = {
                let mut rng = rand::thread_rng();
                let u: f64 = rng.gen();
                -(1f64 - u).ln() / self.batch_frequency as f64
            };
            self.next_batch_time += Duration::from_secs_f64(interarrival_time);
        }
        Ok(batch)
    }

    async fn txn_committed(
        &mut self,
        node: NodeId,
        txns: Vec<Arc<Txn>>,
        blk_height: u64,
    ) -> Result<(), CopycatError> {
        // avoid counting blocks that are
        let chain_info = match self.chain_info.entry(node) {
            Entry::Occupied(e) => e.into_mut(),
            Entry::Vacant(e) => e.insert(ChainInfo {
                chain_length: 0,
                txn_count: HashMap::new(),
                total_committed: 0,
            }),
        };

        assert!(blk_height <= chain_info.chain_length + 1); // make sure we are not skipping over some parents
        for height in blk_height..chain_info.chain_length + 1 {
            chain_info.txn_count.remove(&height);
        }
        chain_info.txn_count.insert(blk_height, txns.len() as u64);
        chain_info.chain_length = blk_height; // blk_height starts with 1
        chain_info.total_committed += txns.len() as u64;

        for txn in txns {
            let hash = txn.compute_id()?;
            let start_time = match self.in_flight.remove(&hash) {
                Some(time) => time,
                None => continue, // unrecognized txn, possibly generated from another node
            };

            let commit_latency = Instant::now() - start_time;
            self.latencies.push(commit_latency.as_secs_f64());
        }

        Ok(())
    }

    fn get_stats(&mut self) -> Stats {
        let (num_committed, chain_length, acc_commit_confidence) = self
            .chain_info
            .values()
            .map(|info| {
                let num_committed = if info.chain_length > 0 {
                    (1..info.chain_length + 1)
                        .map(|height| info.txn_count.get(&height).unwrap())
                        .sum()
                } else {
                    0
                };
                (
                    num_committed,
                    info.chain_length,
                    num_committed as f64 / info.total_committed as f64,
                )
            })
            .reduce(|acc, e| {
                (
                    std::cmp::max(acc.0, e.0),
                    std::cmp::max(acc.1, e.1),
                    (acc.2 + e.2),
                )
            })
            .unwrap_or((0, 0, 0f64));
        let commit_confidence = if self.chain_info.len() == 0 {
            1f64
        } else {
            acc_commit_confidence / self.chain_info.len() as f64
        };

        let latencies = std::mem::replace(&mut self.latencies, vec![]);

        Stats {
            latencies,
            num_committed,
            chain_length,
            commit_confidence,
            inflight_txns: self.in_flight.len(),
        }
    }
}
