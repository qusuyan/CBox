mod avalanche;
mod bitcoin;
mod diem;
mod dummy;

mod chain_replication;
use chain_replication::ChainReplicationDecision;
use tokio_metrics::TaskMonitor;

use crate::consts::DECIDE_DELAY_INTERVAL;
use crate::context::BlkCtx;
use crate::get_report_timer;
use crate::protocol::block::Block;
use crate::protocol::crypto::signature::P2PSignature;
use crate::protocol::crypto::threshold_signature::ThresholdSignature;
use crate::stage::{pass, DelayPool};
use crate::transaction::Txn;
use crate::utils::{CopycatError, NodeId};
use crate::vcores::VCoreGroup;
use crate::{config::ChainConfig, peers::PeerMessenger};

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::Instant;

#[async_trait]
trait Decision: Sync + Send {
    async fn new_tail(
        &mut self,
        src: NodeId,
        new_tail: Vec<(Arc<Block>, Arc<BlkCtx>)>,
    ) -> Result<(), CopycatError>;
    async fn commit_ready(&self) -> Result<(), CopycatError>;
    async fn next_to_commit(&mut self) -> Result<(u64, Vec<Arc<Txn>>), CopycatError>;
    async fn timeout(&self) -> Result<(), CopycatError>;
    async fn handle_timeout(&mut self) -> Result<(), CopycatError>;
    async fn handle_peer_msg(&mut self, src: NodeId, content: Vec<u8>) -> Result<(), CopycatError>;
    fn report(&mut self);
}

fn get_decision(
    id: NodeId,
    p2p_signature: P2PSignature,
    threshold_signature: Arc<dyn ThresholdSignature>,
    config: ChainConfig,
    peer_messenger: Arc<PeerMessenger>,
    pmaker_feedback_send: mpsc::Sender<Vec<u8>>,
    delay: Arc<DelayPool>,
) -> Box<dyn Decision> {
    match config {
        ChainConfig::Dummy { .. } => Box::new(dummy::DummyDecision::new()),
        ChainConfig::Bitcoin { config } => bitcoin::new(id, config),
        ChainConfig::Avalanche { config } => avalanche::new(
            id,
            p2p_signature,
            config,
            peer_messenger,
            pmaker_feedback_send,
            delay,
        ),
        ChainConfig::ChainReplication { .. } => {
            Box::new(ChainReplicationDecision::new(id, config, peer_messenger))
        }
        ChainConfig::Diem { config } => diem::new(
            id,
            p2p_signature,
            threshold_signature,
            config,
            peer_messenger,
            pmaker_feedback_send,
            delay,
        ),
    }
}

pub async fn decision_thread(
    id: NodeId,
    p2p_signature: P2PSignature,
    threshold_signature: Arc<dyn ThresholdSignature>,
    config: ChainConfig,
    peer_messenger: Arc<PeerMessenger>,
    mut peer_consensus_recv: mpsc::Receiver<(NodeId, Vec<u8>)>,
    mut block_ready_recv: mpsc::Receiver<(NodeId, Vec<(Arc<Block>, Arc<BlkCtx>)>)>,
    commit_send: mpsc::Sender<(u64, Vec<Arc<Txn>>)>,
    pmaker_feedback_send: mpsc::Sender<Vec<u8>>,
    core_group: Arc<VCoreGroup>,
    monitor: TaskMonitor,
) {
    pf_info!(id; "decision stage starting...");

    let delay = Arc::new(DelayPool::new());
    let mut insert_delay_time = Instant::now() + DECIDE_DELAY_INTERVAL;

    let mut decision_stage = get_decision(
        id,
        p2p_signature,
        threshold_signature,
        config,
        peer_messenger,
        pmaker_feedback_send.clone(), // keep pmaker_feedback pipe around
        delay.clone(),
    );

    let mut report_timer = get_report_timer();
    let mut blks_sent = 0;
    let mut txns_sent = 0;
    let mut blks_recv = 0;
    let mut txns_recv = 0;
    let mut task_interval = monitor.intervals();

    loop {
        tokio::select! {
            new_tail = block_ready_recv.recv() => {
                // handling new block to be voted
                let _permit = match core_group.acquire().await {
                    Ok(permit) => permit,
                    Err(e) => {
                        pf_error!(id; "failed to acquire allowed concurrency: {:?}", e);
                        continue;
                    }
                };

                let (src, new_tail) = match new_tail {
                    Some(tail) => tail,
                    None => {
                        pf_error!(id; "block_ready pipe closed unexpectedly");
                        return;
                    }
                };

                pf_debug!(id; "got new chain tail from {}: {:?}", src, new_tail);
                for (blk, _) in new_tail.iter() {
                    blks_recv += 1;
                    txns_recv += blk.txns.len();
                }

                if let Err(e) = decision_stage.new_tail(src, new_tail).await {
                    pf_error!(id; "failed to record new chain tail: {:?}", e);
                    continue;
                }
            },

            commit_ready = decision_stage.commit_ready() => {
                // getting blocks to be committed and perform clean up as needed
                let _permit = match core_group.acquire().await {
                    Ok(permit) => permit,
                    Err(e) => {
                        pf_error!(id; "failed to acquire allowed concurrency: {:?}", e);
                        continue;
                    }
                };

                if let Err(e) = commit_ready {
                    pf_error!(id; "waiting for commit ready block failed: {:?}", e);
                    continue;
                }

                let (height, block_to_commit) = match decision_stage.next_to_commit().await {
                    Ok(blk) => blk,
                    Err(e) => {
                        pf_error!(id; "getting commit ready block failed: {:?}", e);
                        continue;
                    }
                };

                pf_debug!(id; "committing new block of length {:?}, height {}", block_to_commit.len(), height);

                blks_sent += 1;
                txns_sent += block_to_commit.len();

                drop(_permit);

                if let Err(e) = commit_send.send((height, block_to_commit)).await {
                    pf_error!(id; "failed to send to commit pipe: {:?}", e);
                    continue;
                }

            },

            _ = decision_stage.timeout() => {
                let _permit = match core_group.acquire().await {
                    Ok(permit) => permit,
                    Err(e) => {
                        pf_error!(id; "failed to acquire allowed concurrency: {:?}", e);
                        continue;
                    }
                };

                if let Err(e) = decision_stage.handle_timeout().await {
                    pf_error!(id; "failed to handle timeout: {:?}", e);
                    continue;
                }
            }

            peer_msg = peer_consensus_recv.recv() => {
                // handling vote messages from peers
                let _permit = match core_group.acquire().await {
                    Ok(permit) => permit,
                    Err(e) => {
                        pf_error!(id; "failed to acquire allowed concurrency: {:?}", e);
                        continue;
                    }
                };

                let (src, msg) =  match peer_msg {
                    Some(msg) => msg,
                    None => {
                        pf_error!(id; "peer consensus recv pipe closed unexpectedly");
                        continue;
                    }
                };

                if let Err(e) = decision_stage.handle_peer_msg(src, msg).await {
                    pf_error!(id; "failed to handle message from peer {}: {:?}", src, e);
                    continue;
                }
            },

            _ = pass(), if Instant::now() > insert_delay_time => {
                tokio::task::yield_now().await;
                insert_delay_time = Instant::now() + DECIDE_DELAY_INTERVAL;
            }

            report_val = report_timer.changed() => {
                if let Err(e) = report_val {
                    pf_error!(id; "Waiting for report timeout failed: {}", e);
                }
                // report basic statistics
                pf_info!(id; "In the last minute: blks_recv: {}, txns_recv: {}, blks_sent: {}, txns_sent: {}", blks_recv, txns_recv, blks_sent, txns_sent);
                decision_stage.report();
                blks_recv = 0;
                txns_recv = 0;
                blks_sent = 0;
                txns_sent = 0;

                let metrics = task_interval.next().unwrap();
                let sched_count = metrics.total_scheduled_count;
                let mean_sched_dur = metrics.mean_scheduled_duration().as_secs_f64();
                let poll_count = metrics.total_poll_count;
                let mean_poll_dur = metrics.mean_poll_duration().as_secs_f64();
                pf_info!(id; "In the last minute: sched_count: {}, mean_sched_dur: {} s, poll_count: {}, mean_poll_dur: {} s", sched_count, mean_sched_dur, poll_count, mean_poll_dur);
            }
        }
    }
}
