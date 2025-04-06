use super::{FlowGen, Stats};
use crate::{ClientId, FlowGenId};
use copycat::protocol::crypto::{Hash, PrivKey, PubKey};
use copycat::transaction::{DiemAccountAddress, DiemPayload};
use copycat::transaction::{DiemTxn, Txn};
use copycat::{CopycatError, NodeId, SignatureScheme};

use async_trait::async_trait;
use rand::Rng;

use std::collections::hash_map::Entry;
use std::collections::HashMap;
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

pub struct DiemFlowGen {
    script_size: usize,
    max_inflight: usize,
    batch_frequency: usize,
    batch_size: usize,
    next_batch_time: Instant,
    crypto: SignatureScheme,
    client_list: Vec<ClientId>,
    accounts: HashMap<ClientId, Vec<(DiemAccountAddress, (PubKey, PrivKey), u64, u64)>>, // addr, (pk, sk), seqno, balance
    in_flight: HashMap<Hash, Instant>,
    chain_info: HashMap<NodeId, ChainInfo>,
    latencies: Vec<f64>,
    _notify: Notify,
}

impl DiemFlowGen {
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
        let mut i = 0u64;
        for client in client_list.iter() {
            for _ in 0..accounts_per_node as u64 {
                let seed = (id << 32) | i;
                i += 1;
                let (pubkey, privkey) = crypto.gen_key_pair(seed);
                let addr = (seed as u128).to_ne_bytes();
                accounts.entry(*client).or_insert(vec![]).push((
                    addr.try_into().unwrap(),
                    (pubkey, privkey),
                    0u64,
                    0u64,
                ));
            }
        }

        let (batch_size, batch_frequency) = if frequency == UNSET {
            (std::cmp::min(max_inflight, 3000), UNSET)
        } else if frequency < MAX_BATCH_FREQ {
            (1, frequency)
        } else {
            (frequency / MAX_BATCH_FREQ, MAX_BATCH_FREQ)
        };

        Self {
            script_size: script_size.unwrap_or(16 + 8), // recver_addr + amount for send txns
            max_inflight,
            batch_frequency,
            batch_size,
            next_batch_time: Instant::now(),
            crypto,
            client_list,
            accounts,
            in_flight: HashMap::new(),
            latencies: vec![],
            chain_info: HashMap::new(),
            _notify: Notify::new(),
        }
    }
}

#[async_trait]
impl FlowGen for DiemFlowGen {
    async fn setup_txns(&mut self) -> Result<Vec<(ClientId, Arc<Txn>)>, CopycatError> {
        let mut txns = Vec::new();
        for (node, accounts) in self.accounts.iter_mut() {
            for (account_addr, (pk, _), _, balance) in accounts.iter_mut() {
                let txn = Txn::Diem {
                    txn: DiemTxn::Grant {
                        receiver: *account_addr,
                        receiver_key: pk.clone(),
                        amount: 10,
                    },
                };
                *balance += 10;
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
            let accounts = self.accounts.get_mut(&node).unwrap();
            let sender_idx = rand::random::<usize>() % accounts.len();
            let (sender_addr, (sender_pk, sender_sk), sender_seq_no, _) =
                accounts.get_mut(sender_idx).unwrap();
            *sender_seq_no += 1;

            let data = (
                *sender_addr,
                *sender_seq_no,
                DiemPayload {
                    script_bytes: self.script_size,
                    script_runtime_sec: 0f64,
                    script_succeed: true,
                    distinct_writes: 1,
                },
                5,
                0,
                // "XUS".to_owned(),
                1611792876,
                4,
            );
            let serialized = bincode::serialize(&data)?;
            let (signature, _) = self.crypto.sign(&sender_sk, &serialized)?;
            let (
                sender,
                seqno,
                payload,
                max_gas_amount,
                gas_unit_price,
                // gas_currency_code,
                expiration_timestamp_secs,
                chain_id,
            ) = data;

            let txn = Arc::new(Txn::Diem {
                txn: DiemTxn::Txn {
                    sender, // address
                    seqno,
                    payload,
                    max_gas_amount,
                    gas_unit_price,
                    // gas_currency_code,
                    expiration_timestamp_secs,
                    chain_id,
                    sender_key: sender_pk.clone(),
                    signature,
                },
            });

            let txn_id = txn.compute_id()?;
            self.in_flight.insert(txn_id, Instant::now());
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
                chain_length: 1,
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
                    (2..info.chain_length + 1)
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
