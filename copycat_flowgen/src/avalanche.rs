use super::{FlowGen, Stats};
use crate::{mock_sign, ClientId, FlowGenId};
use copycat::protocol::crypto::{Hash, PrivKey, PubKey};
use copycat::protocol::transaction::{AvalancheTxn, Txn};
use copycat::{CopycatError, NodeId, SignatureScheme};

use async_trait::async_trait;
use rand::Rng;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Notify;
use tokio::time::{Duration, Instant};

const UNSET: usize = 0;
const MAX_BATCH_FREQ: usize = 100; // 100 batches per sec max, or 1 batch every 10 ms

struct DagInfo {
    num_blks: u64,
    txn_count: u64, // includes false commits
}

pub struct AvalancheFlowGen {
    avax_commit_depth: usize,
    max_inflight: usize,
    batch_frequency: usize,
    batch_size: usize,
    conflict_rate: f64,
    script_size: usize,
    next_batch_time: Instant,
    crypto: SignatureScheme,
    client_list: Vec<ClientId>,
    utxos: HashMap<ClientId, Vec<(usize, Hash, u64)>>, // (utxo_owner_idx, txn_hash, utxo_value)
    accounts: HashMap<ClientId, Vec<(PubKey, PrivKey)>>,
    in_flight: HashMap<Hash, Instant>,
    nonce: u64,
    in_flight_conflict: HashMap<Hash, Hash>,
    conflicting_utxos: Vec<((ClientId, usize), Hash, u64, (ClientId, ClientId))>,
    conflict_set: HashSet<Hash>,
    conflicts_sent: usize,      // pair of conflicting txns sent
    conflicts_recv: usize,      // pair of conflicting txns which I have receied one txn from them
    conflicts_committed: usize, // pair of conflicting txns which I have received both txns
    dag_info: HashMap<NodeId, DagInfo>,
    latencies: Vec<f64>,
    _notify: Notify,
    inflight_snapshot: HashSet<Hash>,
}

impl AvalancheFlowGen {
    pub fn new(
        id: FlowGenId,
        client_list: Vec<ClientId>,
        num_accounts: usize,
        script_size: Option<usize>,
        max_inflight: usize,
        frequency: usize,
        conflict_rate: f64,
        crypto: SignatureScheme,
    ) -> Self {
        let accounts_per_node = if client_list.len() == 0 {
            0
        } else {
            num_accounts / client_list.len()
        };

        assert!(accounts_per_node > 1);

        let mut i = 0u64;
        let mut accounts = HashMap::new();
        let mut utxos = HashMap::new();
        for client in client_list.iter() {
            let mut client_local_accounts = vec![];
            for _ in 0..accounts_per_node as u64 {
                let seed = (id << 32) | i;
                i += 1;
                let (pubkey, privkey) = crypto.gen_key_pair(seed);
                utxos.entry(*client).or_insert(vec![]);
                client_local_accounts.push((pubkey, privkey));
            }
            accounts.insert(*client, client_local_accounts);
        }

        let (batch_size, batch_frequency) = if frequency == UNSET {
            (max_inflight, UNSET)
        } else if frequency < MAX_BATCH_FREQ {
            (1, frequency)
        } else {
            (frequency / MAX_BATCH_FREQ, MAX_BATCH_FREQ)
        };

        Self {
            avax_commit_depth: 50,
            max_inflight,
            batch_frequency,
            batch_size,
            conflict_rate,
            script_size: script_size.unwrap_or(0),
            next_batch_time: Instant::now(),
            crypto,
            client_list,
            utxos,
            accounts,
            in_flight: HashMap::new(),
            nonce: 2,
            in_flight_conflict: HashMap::new(),
            conflicting_utxos: vec![],
            conflict_set: HashSet::new(),
            conflicts_sent: 0,
            conflicts_recv: 0,
            conflicts_committed: 0,
            latencies: vec![],
            dag_info: HashMap::new(),
            _notify: Notify::new(),
            inflight_snapshot: HashSet::new(),
        }
    }

    fn get_inflight_count(&self) -> usize {
        self.in_flight.len() - self.in_flight_conflict.len() + self.conflicts_sent
            - self.conflicts_recv
    }
}

#[async_trait]
impl FlowGen for AvalancheFlowGen {
    async fn setup_txns(&mut self) -> Result<Vec<(ClientId, Arc<Txn>)>, CopycatError> {
        let mut txns = Vec::new();
        for (client, accounts) in self.accounts.iter() {
            for idx in 0..accounts.len() {
                let (pk, _) = &accounts[idx];
                let txn = Arc::new(Txn::Avalanche {
                    txn: AvalancheTxn::Grant {
                        out_utxo: 10,
                        receiver: pk.clone(),
                        nonce: 0,
                    },
                });
                let txn_id = txn.compute_id()?;
                // utxos.push_back((txn_ctx.id, 10));
                self.utxos.get_mut(client).unwrap().push((idx, txn_id, 10));
                txns.push((*client, txn));
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
            if self.max_inflight == UNSET || self.get_inflight_count() < self.max_inflight {
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
            std::cmp::min(
                self.batch_size,
                self.max_inflight - self.get_inflight_count(),
            )
        };
        let mut cur_batch_size = 0;
        while cur_batch_size < batch_size {
            if rand::random::<f64>() > self.conflict_rate {
                let client_idx = rand::random::<usize>() % self.client_list.len();
                let client = self.client_list[client_idx];
                let accounts = self.accounts.get(&client).unwrap();
                let utxos = self.utxos.get_mut(&client).unwrap();

                let send_utxo_idx = rand::random::<usize>() % utxos.len();
                let (sender_idx, in_utxo_raw, in_utxo_amount) = utxos.remove(send_utxo_idx);
                let (sender_pk, sender_sk) = &accounts[sender_idx];

                assert!(accounts.len() > 1);

                let in_utxo = vec![in_utxo_raw];
                let sender_signature = mock_sign(self.crypto, &sender_sk, &in_utxo)?;
                let out_utxo = std::cmp::min(in_utxo_amount, 10);
                let remainder = in_utxo_amount - out_utxo;
                let mut recver_idx = rand::random::<usize>() % (accounts.len() - 1);
                if recver_idx >= sender_idx {
                    recver_idx += 1; // avoid sending to self
                }
                let (recver_pk, _) = &accounts[recver_idx];
                let txn = Arc::new(Txn::Avalanche {
                    txn: AvalancheTxn::Send {
                        sender: sender_pk.clone(),
                        in_utxo,
                        receiver: recver_pk.clone(),
                        out_utxo,
                        remainder,
                        sender_signature,
                        payload_size: self.script_size,
                    },
                });

                let txn_hash = txn.compute_id()?;
                if remainder > 0 {
                    utxos.push((sender_idx, txn_hash.clone(), remainder));
                }
                if out_utxo > 0 {
                    utxos.push((recver_idx, txn_hash.clone(), out_utxo));
                }

                self.in_flight.insert(txn_hash, Instant::now());
                batch.push((client, txn));
                cur_batch_size += 1;
            } else {
                let ((client, sender_idx), in_txn_hash, value, (client1, client2)) =
                    if self.conflicting_utxos.len() > 0 {
                        self.conflicting_utxos
                            .remove(rand::random::<usize>() % self.conflicting_utxos.len())
                    } else {
                        // create a new grant txn
                        let client_idx = rand::random::<usize>() % self.client_list.len();
                        let client = self.client_list[client_idx];
                        let accounts = self.accounts.get(&client).unwrap();
                        let owner_idx = rand::random::<usize>() % accounts.len();
                        let (pk, _) = &accounts[owner_idx];
                        let grant_txn = Arc::new(Txn::Avalanche {
                            txn: AvalancheTxn::Grant {
                                out_utxo: 10,
                                receiver: pk.clone(),
                                nonce: self.nonce,
                            },
                        });
                        self.nonce += 1;
                        let grant_txn_hash = grant_txn.compute_id()?;
                        self.in_flight.insert(grant_txn_hash, Instant::now());

                        let client2_idx = (client_idx + 1) % self.client_list.len();
                        let client2 = self.client_list[client2_idx];

                        batch.push((client, grant_txn.clone()));
                        batch.push((client2, grant_txn.clone()));
                        cur_batch_size += 1;

                        ((client, owner_idx), grant_txn_hash, 10, (client, client2))
                    };

                let accounts = self.accounts.get(&client).unwrap();
                let (sender_pk, sender_sk) = &accounts[sender_idx];

                let in_utxo = vec![in_txn_hash];
                let sender_signature = mock_sign(self.crypto, &sender_sk, &in_utxo)?;

                let mut recver1_idx = rand::random::<usize>() % (accounts.len() - 1);
                if recver1_idx >= sender_idx {
                    recver1_idx += 1; // avoid sending to self
                }
                let (recver1_pk, _) = &accounts[recver1_idx];
                let txn1 = Arc::new(Txn::Avalanche {
                    txn: AvalancheTxn::Send {
                        sender: sender_pk.clone(),
                        in_utxo: in_utxo.clone(),
                        receiver: recver1_pk.clone(),
                        out_utxo: value,
                        remainder: 0,
                        sender_signature: sender_signature.clone(),
                        payload_size: self.script_size,
                    },
                });
                let base_txn1_hash = txn1.compute_id()?;

                let mut recver2_idx = rand::random::<usize>() % (accounts.len() - 2);
                if recver2_idx >= sender_idx {
                    recver2_idx += 1; // avoid sending to self
                }
                if recver2_idx >= recver1_idx {
                    recver2_idx += 1; // avoid sending to the same receiver
                }
                let (recver2_pk, _) = &accounts[recver2_idx];
                let txn2 = Arc::new(Txn::Avalanche {
                    txn: AvalancheTxn::Send {
                        sender: sender_pk.clone(),
                        in_utxo,
                        receiver: recver2_pk.clone(),
                        out_utxo: value,
                        remainder: 0,
                        sender_signature,
                        payload_size: self.script_size,
                    },
                });
                let base_txn2_hash = txn2.compute_id()?;

                self.in_flight.insert(base_txn1_hash, Instant::now());
                self.in_flight.insert(base_txn2_hash, Instant::now());
                self.in_flight_conflict
                    .insert(base_txn1_hash, base_txn2_hash);
                self.in_flight_conflict
                    .insert(base_txn2_hash, base_txn1_hash);
                self.conflicts_sent += 1;

                batch.push((client1, txn1.clone()));
                batch.push((client1, txn2.clone()));
                batch.push((client2, txn2.clone()));
                batch.push((client2, txn1.clone()));
                cur_batch_size += 1;

                // extend both txns enough that they will be committed
                let mut txn1_hash = base_txn1_hash;
                let mut txn2_hash = base_txn2_hash;
                for _ in 0..self.avax_commit_depth {
                    let sender1_idx = recver1_idx;
                    let (sender1_pk, sender1_sk) = &accounts[sender1_idx];
                    recver1_idx = rand::random::<usize>() % (accounts.len() - 1);
                    if recver1_idx >= sender1_idx {
                        recver1_idx += 1;
                    }
                    let (recver1_pk, _) = &accounts[recver1_idx];

                    let in_utxo1 = vec![txn1_hash];
                    let sender_signature1 = mock_sign(self.crypto, &sender1_sk, &in_utxo1)?;
                    let txn1 = Arc::new(Txn::Avalanche {
                        txn: AvalancheTxn::Send {
                            sender: sender1_pk.clone(),
                            in_utxo: in_utxo1,
                            receiver: recver1_pk.clone(),
                            out_utxo: value,
                            remainder: 0,
                            sender_signature: sender_signature1,
                            payload_size: self.script_size,
                        },
                    });
                    txn1_hash = txn1.compute_id()?;

                    let sender2_idx = recver2_idx;
                    let (sender2_pk, sender2_sk) = &accounts[sender2_idx];
                    recver2_idx = rand::random::<usize>() % (accounts.len() - 1);
                    if recver2_idx >= sender2_idx {
                        recver2_idx += 1;
                    }
                    let (recver2_pk, _) = &accounts[recver2_idx];

                    let in_utxo2 = vec![txn2_hash];
                    let sender_signature2 = mock_sign(self.crypto, &sender2_sk, &in_utxo2)?;
                    let txn2 = Arc::new(Txn::Avalanche {
                        txn: AvalancheTxn::Send {
                            sender: sender2_pk.clone(),
                            in_utxo: in_utxo2,
                            receiver: recver2_pk.clone(),
                            out_utxo: value,
                            remainder: 0,
                            sender_signature: sender_signature2,
                            payload_size: self.script_size,
                        },
                    });
                    txn2_hash = txn2.compute_id()?;

                    self.in_flight.insert(txn1_hash, Instant::now());
                    self.in_flight.insert(txn2_hash, Instant::now());
                    self.in_flight_conflict.insert(txn1_hash, txn2_hash);
                    self.in_flight_conflict.insert(txn2_hash, txn1_hash);
                    self.conflicts_sent += 1;

                    batch.push((client1, txn1.clone()));
                    batch.push((client1, txn2.clone()));
                    batch.push((client2, txn2.clone()));
                    batch.push((client2, txn1.clone()));
                    cur_batch_size += 1;
                }

                self.conflicting_utxos.push((
                    (client, recver1_idx),
                    txn1_hash,
                    value,
                    (client1, client2),
                ));
                self.conflicting_utxos.push((
                    (client, recver2_idx),
                    txn2_hash,
                    value,
                    (client1, client2),
                ));
            }
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
        _blk_height: u64,
    ) -> Result<(), CopycatError> {
        // avoid counting blocks that are
        let chain_info = self.dag_info.entry(node).or_insert(DagInfo {
            num_blks: 0,
            txn_count: 0,
        });

        chain_info.num_blks += 1;
        chain_info.txn_count += txns.len() as u64;

        for txn in txns.iter() {
            let hash = txn.compute_id()?;

            // update commit latency
            if let Some(time) = self.in_flight.remove(&hash) {
                let commit_latency = Instant::now() - time;
                self.latencies.push(commit_latency.as_secs_f64());
            };

            // check for conflicts
            if let Some(conflict_hash) = self.in_flight_conflict.remove(&hash) {
                if self.conflict_set.contains(&hash) {
                    log::debug!("Conflicting txns committed");
                    self.conflicts_committed += 1;
                    self.conflict_set.remove(&hash); // to avoid double counting
                } else {
                    self.conflicts_recv += 1;
                    self.conflict_set.insert(conflict_hash);
                }
            }
        }

        Ok(())
    }

    fn get_stats(&mut self) -> Stats {
        let (num_committed, chain_length) = self
            .dag_info
            .values()
            .map(|info| (info.txn_count, info.num_blks))
            .reduce(|acc, e| (std::cmp::max(acc.0, e.0), std::cmp::max(acc.1, e.1)))
            .unwrap_or((0, 0));
        let commit_confidence = if self.conflicts_recv == 0 {
            1f64
        } else {
            1f64 - (self.conflicts_committed as f64 / self.conflicts_recv as f64)
        };

        if cfg!(debug_assertions) {
            let inflight_snapshot: HashSet<_> = self.in_flight.keys().cloned().collect();
            let diff: Vec<_> = self
                .inflight_snapshot
                .intersection(&inflight_snapshot)
                .collect();
            log::info!("txns still inflight since last report: {:?}", diff);
            self.inflight_snapshot = inflight_snapshot;
        }

        let latencies = std::mem::replace(&mut self.latencies, vec![]);

        Stats {
            latencies,
            num_committed,
            chain_length,
            commit_confidence,
            inflight_txns: self.get_inflight_count(),
        }
    }
}
