use super::{FlowGen, Stats};
use crate::{mock_sign, ClientId, FlowGenId, LAT_SAMPLE_RATE};
use copycat::protocol::crypto::{PrivKey, PubKey};
use copycat::transaction::{DiemAccountAddress, DiemPayload};
use copycat::transaction::{DiemTxn, Txn};
use copycat::{CopycatError, NodeId, SignatureScheme};

use async_trait::async_trait;
use rand::seq::SliceRandom;
use rand::Rng;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Notify;
use tokio::time::{Duration, Instant};

const UNSET: usize = 0;
const MAX_BATCH_FREQ: usize = 20;

struct ChainInfo {
    chain_length: u64,
    total_committed: u64, // includes false commits
}

pub struct DiemFlowGen {
    script_size: usize,
    script_runtime_sec: f64,
    max_inflight: usize,
    batch_frequency: usize,
    batch_size: usize,
    next_batch_time: Instant,
    crypto: SignatureScheme,
    client_list: Vec<ClientId>,
    accounts: HashMap<ClientId, Vec<(DiemAccountAddress, (PubKey, PrivKey), u64, u64)>>, // addr, (pk, sk), seqno, balance
    in_flight: HashMap<(DiemAccountAddress, u64), Instant>,
    chain_info: ChainInfo,
    latencies: Vec<f64>,
    _notify: Notify,
}

impl DiemFlowGen {
    pub fn new(
        id: FlowGenId,
        client_list: Vec<ClientId>,
        num_accounts: usize,
        script_size: Option<usize>,
        script_runtime_sec: Option<f64>,
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
            script_runtime_sec: script_runtime_sec.unwrap_or(0f64),
            max_inflight,
            batch_frequency,
            batch_size,
            next_batch_time: Instant::now(),
            crypto,
            client_list,
            accounts,
            in_flight: HashMap::new(),
            latencies: vec![],
            chain_info: ChainInfo {
                chain_length: 1,
                total_committed: 0,
            },
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
            self._notify.notified().await;
        }
    }

    async fn next_txn_batch(&mut self) -> Result<Vec<(ClientId, Arc<Txn>)>, CopycatError> {
        let mut batch = vec![];
        let batch_size = if self.max_inflight == UNSET {
            self.batch_size
        } else {
            std::cmp::min(self.batch_size, self.max_inflight - self.in_flight.len())
        };
        let mut rng = rand::thread_rng();
        for _ in 0..batch_size {
            let node = *self.client_list.choose(&mut rng).unwrap();
            let accounts = self.accounts.get_mut(&node).unwrap();
            let (sender_addr, (sender_pk, sender_sk), sender_seq_no, _) =
                accounts.choose_mut(&mut rng).unwrap();
            *sender_seq_no += 1;

            let payload = DiemPayload {
                script_bytes: self.script_size,
                script_runtime_sec: self.script_runtime_sec,
                script_succeed: true,
                distinct_writes: 1,
            };
            let max_gas_amount = 5;
            let gas_unit_price = 0;
            let expiration_timestamp_secs = 1611792876;
            let chain_id = 4;

            let data = (
                sender_addr.as_ref(),
                *sender_seq_no,
                &payload,
                &max_gas_amount,
                &gas_unit_price,
                // "XUS".to_owned(),
                &expiration_timestamp_secs,
                &chain_id,
            );
            let signature = mock_sign(self.crypto, &sender_sk, &data)?;

            let txn = Arc::new(Txn::Diem {
                txn: DiemTxn::Txn {
                    sender: *sender_addr, // address
                    seqno: *sender_seq_no,
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

            let txn_id = (*sender_addr, *sender_seq_no);
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
        _node: NodeId,
        txns: Vec<Arc<Txn>>,
        blk_height: u64,
    ) -> Result<(), CopycatError> {
        // ignore same block committed by different nodes
        if self.chain_info.chain_length >= blk_height {
            return Ok(());
        } else if self.chain_info.chain_length + 1 != blk_height {
            return Err(CopycatError(format!(
                "DiemFlowGen: blk_height {} is not sequential to {}",
                blk_height, self.chain_info.chain_length
            )));
        }

        self.chain_info.chain_length = blk_height;
        self.chain_info.total_committed += txns.len() as u64;

        let mut rng = rand::thread_rng();
        for txn in txns {
            let txn_id = match txn.as_ref() {
                Txn::Diem {
                    txn: DiemTxn::Txn { sender, seqno, .. },
                } => (*sender, *seqno),
                _ => continue,
            };
            let start_time = match self.in_flight.remove(&txn_id) {
                Some(time) => time,
                None => continue, // unrecognized txn, possibly generated from another node
            };

            if rng.gen::<f64>() < *LAT_SAMPLE_RATE {
                let commit_latency = Instant::now() - start_time;
                self.latencies.push(commit_latency.as_secs_f64());
            }
        }

        Ok(())
    }

    fn get_stats(&mut self) -> Stats {
        let num_latencies = self.latencies.len();
        let latencies = std::mem::replace(&mut self.latencies, Vec::with_capacity(num_latencies));

        Stats {
            latencies,
            num_committed: self.chain_info.total_committed,
            chain_length: self.chain_info.chain_length,
            commit_confidence: 1.0, // since commits in diem are final
            inflight_txns: self.in_flight.len(),
        }
    }
}
