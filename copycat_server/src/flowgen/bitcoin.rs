use super::{FlowGen, Stats};
use copycat_protocol::crypto::{sha256, Hash};
use copycat_protocol::transaction::{BitcoinTxn, Txn};
use copycat_utils::CopycatError;

use async_trait::async_trait;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

const MAX_INFLIGHT: usize = 100000;

pub struct BitcoinFlowGen {
    in_flight: HashMap<Hash, Instant>,
    num_accounts: u128,
    num_completed_txns: u64,
    total_committed_txns: u64,
    total_time_sec: f64,
}

impl BitcoinFlowGen {
    pub fn new() -> Self {
        Self {
            in_flight: HashMap::new(),
            num_accounts: 0,
            num_completed_txns: 0,
            total_time_sec: 0.0,
            total_committed_txns: 0,
        }
    }
}

#[async_trait]
impl FlowGen for BitcoinFlowGen {
    async fn setup_txns(&self) -> Result<Vec<Arc<Txn>>, CopycatError> {
        todo!();
    }

    async fn wait_next(&self) -> Result<(), CopycatError> {
        loop {
            if self.in_flight.len() < MAX_INFLIGHT {
                return Ok(());
            }
            tokio::task::yield_now().await;
        }
    }

    async fn next_txn(&mut self) -> Result<Arc<Txn>, CopycatError> {
        let mut receiver_key = [0u8; 32];
        receiver_key[..16].clone_from_slice(&self.num_accounts.to_ne_bytes());
        self.num_accounts += 1;
        let txn = Txn::Bitcoin {
            txn: BitcoinTxn::Grant {
                out_utxo: 100,
                receiver: receiver_key,
            },
        };
        let serialized = bincode::serialize(&txn)?;
        let hash = sha256(&serialized)?;
        self.in_flight.insert(hash, Instant::now());
        Ok(Arc::new(txn))
    }

    async fn txn_committed(&mut self, txn: Arc<Txn>) -> Result<(), CopycatError> {
        self.total_committed_txns += 1;

        let serialized = bincode::serialize(txn.as_ref())?;
        let hash = sha256(&serialized)?;
        let start_time = match self.in_flight.remove(&hash) {
            Some(time) => time,
            None => return Ok(()), // unrecognized txn, possibly generated from another node
        };

        let commit_latency = Instant::now() - start_time;
        self.total_time_sec += commit_latency.as_secs_f64();
        self.num_completed_txns += 1;

        Ok(())
    }

    fn get_stats(&self) -> Stats {
        let latency = if self.num_completed_txns == 0 {
            0f64
        } else {
            self.total_time_sec / self.num_completed_txns as f64
        };
        Stats {
            latency,
            num_committed: self.total_committed_txns,
        }
    }
}
