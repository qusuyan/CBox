use super::{FlowGen, Stats};
use crate::{ClientId, FlowGenId};
use copycat::protocol::crypto::{Hash, PrivKey, PubKey};
use copycat::protocol::transaction::{AvalancheTxn, Txn};
use copycat::{CopycatError, CryptoScheme, NodeId, TxnCtx};

use async_trait::async_trait;
use rand::Rng;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Notify;
use tokio::time::{Duration, Instant};

const UNSET: usize = 0;
const MAX_BATCH_FREQ: usize = 100; // 100 batches per sec max, or 1 batch every 10 ms

struct DagInfo {
    num_blks: u64,
    txn_count: HashMap<u64, u64>,
    total_committed: u64, // includes false commits
}

pub struct AvalancheFlowGen {
    max_inflight: usize,
    batch_frequency: usize,
    batch_size: usize,
    next_batch_time: Instant,
    crypto: CryptoScheme,
    client_list: Vec<ClientId>,
    utxos: HashMap<ClientId, Vec<(usize, Hash, u64)>>,
    accounts: HashMap<ClientId, Vec<(PubKey, PrivKey)>>,
    in_flight: HashMap<Hash, Instant>,
    num_completed_txns: u64,
    dag_info: HashMap<NodeId, DagInfo>,
    total_time_sec: f64,
    _notify: Notify,
}

impl AvalancheFlowGen {
    pub fn new(
        id: FlowGenId,
        client_list: Vec<ClientId>,
        num_accounts: usize,
        max_inflight: usize,
        frequency: usize,
        crypto: CryptoScheme,
    ) -> Self {
        let accounts_per_node = if client_list.len() == 0 {
            0
        } else {
            num_accounts / client_list.len()
        };

        let mut i = 0u64;
        let mut accounts = HashMap::new();
        let mut utxos = HashMap::new();
        for client in client_list.iter() {
            for _ in 0..accounts_per_node as u64 {
                let seed = (id << 32) | i;
                i += 1;
                let (pubkey, privkey) = crypto.gen_key_pair(seed);
                utxos.entry(*client).or_insert(vec![]);
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
            next_batch_time: Instant::now(),
            crypto,
            client_list,
            utxos,
            accounts,
            in_flight: HashMap::new(),
            num_completed_txns: 0,
            total_time_sec: 0.0,
            dag_info: HashMap::new(),
            _notify: Notify::new(),
        }
    }
}

#[async_trait]
impl FlowGen for AvalancheFlowGen {
    async fn setup_txns(&mut self) -> Result<Vec<(ClientId, Arc<Txn>)>, CopycatError> {
        let mut txns = Vec::new();
        for (node, accounts) in self.accounts.iter() {
            for idx in 0..accounts.len() {
                let (pk, _) = &accounts[idx];
                let txn = Txn::Avalanche {
                    txn: AvalancheTxn::Grant {
                        out_utxo: 10,
                        receiver: pk.clone(),
                    },
                };
                let txn_ctx = TxnCtx::from_txn(&txn)?;
                // utxos.push_back((txn_ctx.id, 10));
                self.utxos
                    .get_mut(node)
                    .unwrap()
                    .push((idx, txn_ctx.id, 10));
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

            let send_utxo_idx = rand::random::<usize>() % utxos.len();
            let (sender_idx, in_utxo_raw, in_utxo_amount) = utxos.remove(send_utxo_idx);
            let (sender_pk, sender_sk) = &accounts[sender_idx];

            let mut recver_idx = rand::random::<usize>() % (accounts.len() - 1);
            if recver_idx >= sender_idx {
                recver_idx += 1; // avoid sending to self
            }
            let (recver_pk, _) = &accounts[recver_idx];

            let in_utxo = vec![in_utxo_raw];
            let serialized_in_utxo = bincode::serialize(&in_utxo)?;
            let (sender_signature, _) = self.crypto.sign(&sender_sk, &serialized_in_utxo)?;
            let out_utxo = std::cmp::min(in_utxo_amount, 10);
            let remainder = in_utxo_amount - out_utxo;
            let txn = Arc::new(Txn::Avalanche {
                txn: AvalancheTxn::Send {
                    sender: sender_pk.clone(),
                    in_utxo,
                    receiver: recver_pk.clone(),
                    out_utxo,
                    remainder,
                    sender_signature,
                },
            });

            let txn_ctx = TxnCtx::from_txn(&txn)?;
            let txn_hash = txn_ctx.id;
            if remainder > 0 {
                utxos.push((sender_idx, txn_hash.clone(), remainder));
            }
            if out_utxo > 0 {
                utxos.push((recver_idx, txn_hash.clone(), out_utxo));
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
        let chain_info = self.dag_info.entry(node).or_insert(DagInfo {
            num_blks: 0,
            txn_count: HashMap::new(),
            total_committed: 0,
        });

        if let Some(_) = chain_info.txn_count.insert(blk_height, txns.len() as u64) {
        } else {
            chain_info.num_blks += 1;
        }
        chain_info.total_committed += txns.len() as u64;

        for txn in txns {
            let txn_ctx = TxnCtx::from_txn(&txn)?;
            let hash = txn_ctx.id;
            let start_time = match self.in_flight.remove(&hash) {
                Some(time) => time,
                None => {
                    // if let Txn::Avalanche { txn: avax_txn } = txn.as_ref() {
                    //     if matches!(avax_txn, AvalancheTxn::Send { .. }) {
                    //         log::warn!("unrecognized txn");
                    //     }
                    // }
                    continue;
                } // unrecognized txn, possibly generated from another node
            };

            let commit_latency = Instant::now() - start_time;
            self.total_time_sec += commit_latency.as_secs_f64();
            self.num_completed_txns += 1;
        }

        Ok(())
    }

    fn get_stats(&self) -> Stats {
        let latency = if self.num_completed_txns == 0 {
            0f64
        } else {
            self.total_time_sec / self.num_completed_txns as f64
        };
        let (num_committed, chain_length, acc_commit_confidence) = self
            .dag_info
            .values()
            .map(|info| {
                // since blocks can get committed out of order
                let num_committed = info.txn_count.values().sum();
                let commit_confidence = if info.total_committed == 0 {
                    1f64
                } else {
                    num_committed as f64 / info.total_committed as f64
                };
                (num_committed, info.num_blks, commit_confidence)
            })
            .reduce(|acc, e| {
                (
                    std::cmp::max(acc.0, e.0),
                    std::cmp::max(acc.1, e.1),
                    (acc.2 + e.2),
                )
            })
            .unwrap_or((0, 0, 0f64));
        let commit_confidence = if self.dag_info.len() == 0 {
            1f64
        } else {
            acc_commit_confidence / self.dag_info.len() as f64
        };
        Stats {
            latency,
            num_committed,
            chain_length,
            commit_confidence,
            inflight_txns: self.in_flight.len(),
        }
    }
}
