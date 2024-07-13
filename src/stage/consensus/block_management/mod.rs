mod dummy;
use dummy::DummyBlockManagement;

mod bitcoin;
use bitcoin::BitcoinBlockManagement;

mod avalanche;
use avalanche::AvalancheBlockManagement;

mod chain_replication;
use chain_replication::ChainReplicationBlockManagement;
use tokio_metrics::TaskMonitor;

use crate::config::Config;
use crate::context::{BlkCtx, TxnCtx};
use crate::peers::PeerMessenger;
use crate::protocol::block::Block;
use crate::protocol::transaction::Txn;
use crate::protocol::CryptoScheme;
use crate::utils::{CopycatError, NodeId};

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::{mpsc, Semaphore};
use tokio::time::{Duration, Instant};

use dashmap::DashSet;

use atomic_float::AtomicF64;
use std::sync::atomic::Ordering;

enum CurBlockState {
    Working,
    Full,
    EmptyMempool,
}

#[async_trait]
trait BlockManagement: Sync + Send {
    async fn record_new_txn(
        &mut self,
        txn: Arc<Txn>,
        ctx: Arc<TxnCtx>,
    ) -> Result<bool, CopycatError>;
    async fn prepare_new_block(&mut self) -> Result<CurBlockState, CopycatError>; // return a bool indicating if the block is full
    async fn wait_to_propose(&self) -> Result<(), CopycatError>;
    async fn get_new_block(&mut self) -> Result<(Arc<Block>, Arc<BlkCtx>), CopycatError>;
    async fn validate_block(
        &mut self,
        block: Arc<Block>,
        ctx: Arc<BlkCtx>,
    ) -> Result<Vec<(Arc<Block>, Arc<BlkCtx>)>, CopycatError>;
    async fn handle_pmaker_msg(&mut self, msg: Vec<u8>) -> Result<(), CopycatError>;
    async fn handle_peer_blk_req(&mut self, peer: NodeId, msg: Vec<u8>)
        -> Result<(), CopycatError>;
    fn report(&mut self);
    // async fn handle_peer_blk_resp(
    //     &mut self,
    //     peer: NodeId,
    //     blk_id: Hash,
    //     block: Arc<Block>,
    // ) -> Result<(), CopycatError>;
}

fn get_blk_creation(
    id: NodeId,
    config: Config,
    peer_messenger: Arc<PeerMessenger>,
) -> Box<dyn BlockManagement> {
    match config {
        Config::Dummy { .. } => Box::new(DummyBlockManagement::new()),
        Config::Bitcoin { config } => {
            Box::new(BitcoinBlockManagement::new(id, config, peer_messenger))
        }
        Config::Avalanche { config } => {
            Box::new(AvalancheBlockManagement::new(id, config, peer_messenger))
        }
        Config::ChainReplication { config } => {
            Box::new(ChainReplicationBlockManagement::new(id, config))
        }
    }
}

pub async fn block_management_thread(
    id: NodeId,
    config: Config,
    crypto_scheme: CryptoScheme,
    mut peer_blk_recv: mpsc::Receiver<(NodeId, Arc<Block>)>,
    mut peer_blk_req_recv: mpsc::Receiver<(NodeId, Vec<u8>)>,
    // mut peer_blk_resp_recv: mpsc::Receiver<(NodeId, (Hash, Arc<Block>))>,
    peer_messenger: Arc<PeerMessenger>,
    mut txn_ready_recv: mpsc::Receiver<Vec<(Arc<Txn>, Arc<TxnCtx>)>>,
    mut pacemaker_recv: mpsc::Receiver<Vec<u8>>,
    new_block_send: mpsc::Sender<(NodeId, Vec<(Arc<Block>, Arc<BlkCtx>)>)>,
    concurrency: Arc<Semaphore>,
    monitor: TaskMonitor,
) {
    pf_info!(id; "block management stage starting...");

    let delay = Arc::new(AtomicF64::new(0f64));
    let insert_delay_interval = Duration::from_millis(50);
    let mut insert_delay_time = Instant::now() + insert_delay_interval;

    let mut block_management_stage = get_blk_creation(id, config, peer_messenger);

    let batch_prepare_timeout = Duration::from_millis(1);
    let mut batch_prepare_time = Instant::now() + batch_prepare_timeout;
    let mut blk_state = CurBlockState::Working;

    let (pending_blk_sender, mut pending_blk_recver) = mpsc::channel(0x100000);

    let txns_validated = Arc::new(DashSet::new());

    let mut report_timeout = Instant::now() + Duration::from_secs(60);
    let mut self_txns_sent = 0;
    let mut self_blks_sent = 0;
    let mut peer_txns_sent = 0;
    let mut peer_blks_sent = 0;
    let mut txns_recv = 0;
    let mut peer_blks_recv = 0;
    let mut peer_txns_recv = 0;
    let mut task_interval = monitor.intervals();

    loop {
        tokio::select! {
            new_txn = txn_ready_recv.recv() => {
                // check if transaction is valid based on log history
                let _permit = match concurrency.acquire().await {
                    Ok(permit) => permit,
                    Err(e) => {
                        pf_error!(id; "failed to acquire allowed concurrency: {:?}", e);
                        continue;
                    }
                };
                match new_txn {
                    Some(txn_batch) => {
                        for (txn, ctx) in txn_batch {
                            pf_trace!(id; "got new txn {:?} {:?}", ctx, txn);
                            txns_recv += 1;
                            txns_validated.insert(ctx.id);
                            if let Err(e) = block_management_stage.record_new_txn(txn, ctx).await {
                                pf_error!(id; "failed to record new txn: {:?}", e);
                                continue;
                            }
                        }
                    },
                    None => {
                        pf_error!(id; "txn_ready pipe closed unexpectedly");
                        return;
                    }
                }

                if matches!(blk_state, CurBlockState::EmptyMempool) {
                    blk_state = CurBlockState::Working;
                }
            },

            _ = tokio::time::sleep_until(batch_prepare_time), if matches!(blk_state, CurBlockState::Working) => {
                // preparing for the block to be proposed
                let _permit = match concurrency.acquire().await {
                    Ok(permit) => permit,
                    Err(e) => {
                        pf_error!(id; "failed to acquire allowed concurrency: {:?}", e);
                        continue;
                    }
                };
                match block_management_stage.prepare_new_block().await {
                    Ok(blk_full) => blk_state = blk_full,
                    Err(e) => pf_error!(id; "failed to prepare new block: {:?}", e),
                }
                batch_prepare_time = Instant::now() + batch_prepare_timeout;
            },

            wait_result = block_management_stage.wait_to_propose() => {
                // packing prepared data into an actual block
                let _permit = match concurrency.acquire().await {
                    Ok(permit) => permit,
                    Err(e) => {
                        pf_error!(id; "failed to acquire allowed concurrency: {:?}", e);
                        continue;
                    }
                };
                if let Err(e) = wait_result {
                    pf_error!(id; "wait to propose failed: {:?}", e);
                    continue;
                }

                let (new_blk, new_blk_ctx) = match block_management_stage.get_new_block().await {
                    Ok(blk_with_ctx) => blk_with_ctx,
                    Err(e) => {
                        pf_error!(id; "error creating new block: {:?}", e);
                        continue;
                    }
                };

                pf_debug!(id; "proposing new block {:?}", new_blk);
                blk_state = CurBlockState::Working;
                self_blks_sent += 1;
                self_txns_sent += new_blk.txns.len();

                drop(_permit);

                if let Err(e) = new_block_send.send((id, vec![(new_blk, new_blk_ctx)])).await {
                    pf_error!(id; "failed to send to new_block pipe: {:?}", e);
                    continue;
                }
            },

            peer_blk = peer_blk_recv.recv() => {
                let (src, new_block) = match peer_blk {
                    Some((src, blk)) => {
                        if src == id {
                            // ignore blocks proposed by myself
                            continue;
                        }
                        (src, blk)
                    }
                    None => {
                        pf_error!(id; "peer_blk pipe closed unexpectedly");
                        return;
                    }
                };

                pf_debug!(id; "got from {} new block {:?}, computing its context...", src, new_block);

                let blk_sender = pending_blk_sender.clone();
                let txns_validated = txns_validated.clone();
                let sem = concurrency.clone();
                tokio::task::spawn(async move {
                    // validating txns in peer blocks
                    let _permit = match sem.acquire().await {
                        Ok(permit) => permit,
                        Err(e) => {
                            pf_error!(id; "failed to acquire allowed concurrency: {:?}", e);
                            return;
                        }
                    };
                    let blk_ctx = match BlkCtx::from_blk(&new_block) {
                        Ok(ctx) => ctx,
                        Err(e) => {
                            pf_error!(id; "failed to compute blk_context: {:?}", e);
                            return;
                        }
                    };

                    let mut verification_time = 0f64;
                    for idx in 0..new_block.txns.len() {
                        let txn = &new_block.txns[idx];
                        let txn_ctx = &blk_ctx.txn_ctx[idx];

                        if txns_validated.contains(&txn_ctx.id) {
                            continue;
                        }

                        let (valid, vtime) = match txn.validate(crypto_scheme) {
                            Ok(validity) => validity,
                            Err(e) => {
                                pf_error!(id; "txn validation failed: {:?}", e);
                                continue;
                            }
                        };
                        verification_time += vtime;
                        if valid {
                            txns_validated.insert(txn_ctx.id);
                        } else {
                            // invalid block
                            tokio::time::sleep(Duration::from_secs_f64(verification_time)).await;
                            return;
                        }
                    }

                    tokio::time::sleep(Duration::from_secs_f64(verification_time)).await;

                    drop(_permit);

                    if let Err(e) = blk_sender.send((src, new_block, Arc::new(blk_ctx))).await {
                        pf_error!(id; "failed to send blk_context: {:?}", e);
                    }
                });
            }

            peer_blk_with_ctx = pending_blk_recver.recv() => {
                // validating new block from peer
                let _permit = match concurrency.acquire().await {
                    Ok(permit) => permit,
                    Err(e) => {
                        pf_error!(id; "failed to acquire allowed concurrency: {:?}", e);
                        continue;
                    }
                };
                let (src, peer_blk, peer_blk_ctx) = match peer_blk_with_ctx {
                    Some(data) => data,
                    None => {
                        pf_error!(id; "pending_blk pipe closed unexpectedly");
                        return;
                    }
                };

                peer_blks_recv += 1;
                peer_txns_recv += peer_blk.txns.len();
                pf_debug!(id; "Validating block ({:?}, {:?})", peer_blk, peer_blk_ctx);
                let new_tail = match block_management_stage.validate_block(peer_blk, peer_blk_ctx).await {
                    Ok(new_tail) => new_tail,
                    Err(e) => {
                        pf_error!(id; "failed to validate block: {:?}", e);
                        continue;
                    }
                };

                if new_tail.is_empty() {
                    // don't need to change anything
                    continue;
                }

                blk_state = CurBlockState::Working;
                for (blk, _) in new_tail.iter() {
                    peer_blks_sent += 1;
                    peer_txns_sent += blk.txns.len();
                }

                drop(_permit);

                if let Err(e) = new_block_send.send((src, new_tail)).await {
                    pf_error!(id; "failed to send to block_ready pipe: {:?}", e);
                    continue;
                }
            }

            pmaker_msg = pacemaker_recv.recv() => {
                // handling pmaker message
                let _permit = match concurrency.acquire().await {
                    Ok(permit) => permit,
                    Err(e) => {
                        pf_error!(id; "failed to acquire allowed concurrency: {:?}", e);
                        continue;
                    }
                };
                match pmaker_msg {
                    Some(msg) => {
                        pf_debug!(id; "got pmaker msg");
                        if let Err(e) = block_management_stage.handle_pmaker_msg(msg).await {
                            pf_error!(id; "failed to handle pacemaker message: {:?}", e);
                            continue;
                        }
                    },
                    None => {
                        pf_error!(id; "pacemaker pipe closed unexpectedly");
                        return;
                    }
                }
            }

            req = peer_blk_req_recv.recv() => {
                // handling block requests from peers
                let _permit = match concurrency.acquire().await {
                    Ok(permit) => permit,
                    Err(e) => {
                        pf_error!(id; "failed to acquire allowed concurrency: {:?}", e);
                        continue;
                    }
                };
                match req {
                    Some((peer, msg)) => {
                        pf_debug!(id; "got block request {:?}", msg);
                        if let Err(e) = block_management_stage.handle_peer_blk_req(peer, msg).await {
                            pf_error!(id; "error handling peer block request: {:?}", e);
                            continue;
                        }
                    }
                    None => {
                        pf_error!(id; "peer_blk_req pipe closed unexpectedly");
                        continue;
                    }
                }
            }

            _ = tokio::time::sleep_until(insert_delay_time) => {
                // insert delay as appropriate
                let sleep_time = delay.load(Ordering::Relaxed);
                if sleep_time > 0.05 {
                    // doing skipped compute cost
                    let _permit = match concurrency.acquire().await {
                        Ok(permit) => permit,
                        Err(e) => {
                            pf_error!(id; "failed to acquire allowed concurrency: {:?}", e);
                            continue;
                        }
                    };
                    tokio::time::sleep(Duration::from_secs_f64(sleep_time)).await;
                    delay.store(0f64, Ordering::Relaxed);
                } else {
                    tokio::task::yield_now().await;
                }
                insert_delay_time = Instant::now() + insert_delay_interval;
            }

            _ = tokio::time::sleep_until(report_timeout) => {
                // report basic statistics
                pf_info!(id; "In the last minute: txns_recv: {}, peer_blks_recv: {}, peer_txns_recv: {}", txns_recv, peer_blks_recv, peer_txns_recv);
                pf_info!(id; "In the last minute: self_blks_sent: {}, self_txns_sent: {}, peer_blks_sent: {}, peer_txns_sent: {}", self_blks_sent, self_txns_sent, peer_blks_sent, peer_txns_sent);
                block_management_stage.report();
                txns_recv = 0;
                self_blks_sent = 0;
                self_txns_sent = 0;
                peer_blks_sent = 0;
                peer_txns_sent = 0;
                peer_blks_recv = 0;
                peer_txns_recv = 0;

                let metrics = task_interval.next().unwrap();
                let sched_count = metrics.total_scheduled_count;
                let mean_sched_dur = metrics.mean_scheduled_duration().as_secs_f64();
                let poll_count = metrics.total_poll_count;
                let mean_poll_dur = metrics.mean_poll_duration().as_secs_f64();
                pf_info!(id; "In the last minute: sched_count: {}, mean_sched_dur: {} s, poll_count: {}, mean_poll_dur: {} s", sched_count, mean_sched_dur, poll_count, mean_poll_dur);

                // reset report time
                report_timeout = Instant::now() + Duration::from_secs(60);
            }
        }
    }
}
