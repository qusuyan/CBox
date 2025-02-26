use super::{FlowGen, Stats};
use crate::ClientId;
use copycat::protocol::crypto::{Hash, PubKey, Signature};
use copycat::protocol::transaction::{DummyTxn, Txn};
use copycat::{CopycatError, SignatureScheme, NodeId};

use async_trait::async_trait;
use rand::Rng;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Notify;
use tokio::time::{Duration, Instant};

const UNSET: usize = 0;
const MAX_BATCH_FREQ: usize = 100; // 100 batches per sec max, or 1 batch every 10 ms

struct LogInfo {
    pub num_blks: u64,
    pub num_txns: u64,
}

pub struct ChainReplicationFlowGen {
    // fields to generate load
    max_inflight: usize,
    batch_frequency: usize,
    batch_size: usize,
    next_batch_time: Instant,
    client_list: Vec<ClientId>,
    next_txn_id: Hash,
    in_flight: HashMap<Hash, Instant>,
    // fields to create transactions
    content: Arc<Vec<u8>>, // for simplicity, all txns are the same
    pub_key: PubKey,
    signature: Signature,
    // statistics
    stats: HashMap<NodeId, LogInfo>,
    latencies: Vec<f64>,
    _notify: Notify,
}

impl ChainReplicationFlowGen {
    pub fn new(
        client_list: Vec<ClientId>,
        txn_size: usize,
        max_inflight: usize,
        frequency: usize,
        crypto: SignatureScheme,
    ) -> Self {
        let (batch_size, batch_frequency) = if frequency == UNSET {
            (max_inflight, UNSET)
        } else if frequency < MAX_BATCH_FREQ {
            (1, frequency)
        } else {
            (frequency / MAX_BATCH_FREQ, MAX_BATCH_FREQ)
        };

        let txn_content = Arc::new(vec![0u8; txn_size]);
        let (pub_key, priv_key) = crypto.gen_key_pair(0);
        let signature = crypto.sign(&priv_key, &txn_content).unwrap().0;

        Self {
            max_inflight,
            batch_frequency,
            batch_size,
            next_batch_time: Instant::now(),
            client_list,
            next_txn_id: Hash::zero(),
            in_flight: HashMap::new(),
            content: txn_content,
            pub_key,
            signature,
            stats: HashMap::new(),
            latencies: vec![],
            _notify: Notify::new(),
        }
    }
}

#[async_trait]
impl FlowGen for ChainReplicationFlowGen {
    async fn setup_txns(&mut self) -> Result<Vec<(ClientId, Arc<Txn>)>, CopycatError> {
        Ok(vec![])
    }

    async fn wait_next(&self) -> Result<(), CopycatError> {
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
            let client_id = self.client_list[rand::random::<usize>() % self.client_list.len()];
            let txn = Arc::new(Txn::Dummy {
                txn: DummyTxn {
                    id: self.next_txn_id,
                    content: self.content.clone(),
                    pub_key: self.pub_key.clone(),
                    signature: self.signature.clone(),
                },
            });
            let txn_hash = txn.compute_id()?;

            self.next_txn_id += Hash::one();
            self.in_flight.insert(txn_hash, Instant::now());
            batch.push((client_id, txn));
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
        let log_info = self.stats.entry(node).or_insert(LogInfo {
            num_blks: 0,
            num_txns: 0,
        });
        assert!(blk_height == log_info.num_blks as u64 + 1);
        log_info.num_blks += 1;
        log_info.num_txns += txns.len() as u64;

        for txn in txns {
            let hash = txn.compute_id()?;
            let start_time = match self.in_flight.remove(&hash) {
                Some(time) => time,
                None => {
                    continue;
                } // unrecognized txn, possibly generated from another node
            };

            let commit_latency = Instant::now() - start_time;
            self.latencies.push(commit_latency.as_secs_f64());
        }

        Ok(())
    }

    fn get_stats(&mut self) -> Stats {
        let (num_committed, chain_length) = self
            .stats
            .values()
            .map(|info| (info.num_txns, info.num_blks))
            .reduce(|acc, e| (std::cmp::max(acc.0, e.0), std::cmp::max(acc.1, e.1)))
            .unwrap_or((0, 0));

        let latencies = std::mem::replace(&mut self.latencies, vec![]);

        Stats {
            latencies,
            num_committed,
            chain_length,
            commit_confidence: 1.0, // since non BFT
            inflight_txns: self.in_flight.len(),
        }
    }
}
