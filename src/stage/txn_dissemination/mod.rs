mod broadcast;
use broadcast::BroadcastTxnDissemination;

mod gossip;
use gossip::GossipTxnDissemination;

mod passthrough;
use passthrough::PassthroughTxnDissemination;

use crate::context::TxnCtx;
use crate::protocol::transaction::Txn;
use crate::protocol::DissemPattern;
use crate::utils::{CopycatError, NodeId};
use crate::{config::Config, peers::PeerMessenger};

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::{mpsc, Semaphore};
use tokio::time::{Duration, Instant};

use atomic_float::AtomicF64;
use std::sync::atomic::Ordering;

#[async_trait]
trait TxnDissemination: Send + Sync {
    async fn disseminate(&self, txn_batch: &Vec<(u64, Arc<Txn>)>) -> Result<(), CopycatError>;
}

fn get_txn_dissemination(
    id: NodeId,
    enabled: bool,
    config: Config,
    peer_messenger: Arc<PeerMessenger>,
) -> Box<dyn TxnDissemination> {
    if !enabled {
        return Box::new(PassthroughTxnDissemination::new());
    }

    match config.get_txn_dissem() {
        DissemPattern::Broadcast => Box::new(BroadcastTxnDissemination::new(id, peer_messenger)),
        DissemPattern::Gossip => Box::new(GossipTxnDissemination::new(id, peer_messenger)),
        DissemPattern::Sample => Box::new(BroadcastTxnDissemination::new(id, peer_messenger)), // TODO: use gossip for now
        DissemPattern::Passthrough => Box::new(PassthroughTxnDissemination::new()),
        DissemPattern::Linear => Box::new(PassthroughTxnDissemination::new()), // TODO: use passthrough for now
    }
}

pub async fn txn_dissemination_thread(
    id: NodeId,
    config: Config,
    enabled: bool,
    peer_messenger: Arc<PeerMessenger>,
    mut validated_txn_recv: mpsc::Receiver<Vec<(NodeId, (Arc<Txn>, Arc<TxnCtx>))>>,
    txn_ready_send: mpsc::Sender<Vec<(Arc<Txn>, Arc<TxnCtx>)>>,
    concurrency: Arc<Semaphore>,
) {
    pf_info!(id; "txn dissemination stage starting...");

    let delay = Arc::new(AtomicF64::new(0f64));
    let insert_delay_interval = Duration::from_millis(50);
    let mut insert_delay_time = Instant::now() + insert_delay_interval;

    let txn_dissemination_stage = get_txn_dissemination(id, enabled, config, peer_messenger);
    let mut batch = vec![];
    let mut txn_dissem_time = Instant::now() + Duration::from_millis(100);

    async fn wait_send_batch(batch: &Vec<(u64, (Arc<Txn>, Arc<TxnCtx>))>, timeout: Instant) {
        let notify = tokio::sync::Notify::new();
        if batch.len() == 0 {
            notify.notified().await;
        } else if batch.len() > 2000 {
            return;
        }
        tokio::time::sleep_until(timeout).await;
    }

    let mut report_timeout = Instant::now() + Duration::from_secs(60);

    loop {
        tokio::select! {
            new_txn = validated_txn_recv.recv() => {
                let _ = match concurrency.acquire().await {
                    Ok(permit) => permit,
                    Err(e) => {
                        pf_error!(id; "failed to acquire allowed concurrency: {:?}", e);
                        continue;
                    }
                };

                let mut txn_batch = match new_txn {
                    Some(txn) => txn,
                    None => {
                        pf_error!(id; "validated_txn pipe closed unexpectedly");
                        return;
                    }
                };

                pf_trace!(id; "got new txn batch {:?}", txn_batch);
                batch.append(&mut txn_batch);
            }

            _ = wait_send_batch(&batch, txn_dissem_time) => {
                let _ = match concurrency.acquire().await {
                    Ok(permit) => permit,
                    Err(e) => {
                        pf_error!(id; "failed to acquire allowed concurrency: {:?}", e);
                        continue;
                    }
                };

                let send_batch: Vec<(u64, (Arc<Txn>, Arc<TxnCtx>))> = batch.drain(0..).collect();
                let txns = send_batch.iter().map(|(src, (txn, _))| (*src, txn.clone())).collect();
                if let Err(e) = txn_dissemination_stage.disseminate(&txns).await {
                    pf_error!(id; "failed to disseminate txn: {:?}", e);
                    continue;
                }

                let batched_txns = send_batch.into_iter().map(|(_, txn_with_ctx)| txn_with_ctx).collect();
                if let Err(e) = txn_ready_send.send(batched_txns).await {
                    pf_error!(id; "failed to send to txn_ready pipe: {:?}", e);
                    continue;
                }

                txn_dissem_time = Instant::now() + Duration::from_millis(100);
            }
            _ = tokio::time::sleep_until(insert_delay_time) => {
                // insert delay as appropriate
                let sleep_time = delay.load(Ordering::Relaxed);
                if sleep_time > 0.05 {
                    let _ = match concurrency.acquire().await {
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
                // reset report time
                report_timeout = Instant::now() + Duration::from_secs(60);
            }
        }
    }
}
