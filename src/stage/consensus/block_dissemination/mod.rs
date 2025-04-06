mod broadcast;
use broadcast::BroadcastBlockDissemination;

mod gossip;
use gossip::GossipBlockDissemination;

mod sampling;
use sampling::SamplingBlockDissemination;

mod linear;
use linear::LinearBlockDissemination;

mod passthrough;
use passthrough::PassthroughBlockDissemination;

mod narwhal;
use narwhal::NarwhalBlockDissemination;

use crate::consts::BLK_DISS_DELAY_INTERVAL;
use crate::context::BlkCtx;
use crate::peers::PeerMessenger;
use crate::protocol::block::Block;
use crate::protocol::crypto::threshold_signature::ThresholdSignature;
use crate::protocol::DissemPattern;
use crate::stage::{pass, DelayPool};
use crate::utils::{CopycatError, NodeId};
use crate::vcores::VCoreGroup;
use crate::{get_report_timer, ChainConfig};

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio_metrics::TaskMonitor;

#[async_trait]
trait BlockDissemination: Sync + Send {
    async fn disseminate(
        &mut self,
        src: NodeId,
        blocks: Vec<(Arc<Block>, Arc<BlkCtx>)>,
    ) -> Result<(), CopycatError>;
    async fn wait_disseminated(&self) -> Result<(), CopycatError>;
    async fn get_disseminated(
        &mut self,
    ) -> Result<(NodeId, Vec<(Arc<Block>, Arc<BlkCtx>)>), CopycatError>;
    async fn handle_peer_msg(&mut self, src: NodeId, msg: Vec<u8>) -> Result<(), CopycatError>;
}

fn get_block_dissemination(
    id: NodeId,
    config: ChainConfig,
    peer_messenger: Arc<PeerMessenger>,
    threshold_signature: Arc<dyn ThresholdSignature>,
    pmaker_feedback_send: mpsc::Sender<Vec<u8>>,
    delay: Arc<DelayPool>,
) -> Box<dyn BlockDissemination> {
    match config.get_blk_dissem() {
        DissemPattern::Broadcast => Box::new(BroadcastBlockDissemination::new(id, peer_messenger)),
        DissemPattern::Gossip => Box::new(GossipBlockDissemination::new(id, peer_messenger)),
        DissemPattern::Sample { sample_size } => Box::new(SamplingBlockDissemination::new(
            id,
            peer_messenger,
            sample_size,
        )),
        DissemPattern::Linear { order } => {
            Box::new(LinearBlockDissemination::new(id, peer_messenger, order))
        }
        DissemPattern::Passthrough => {
            Box::new(PassthroughBlockDissemination::new(id, peer_messenger))
        }
        DissemPattern::Narwhal => Box::new(NarwhalBlockDissemination::new(
            id,
            peer_messenger,
            threshold_signature,
            pmaker_feedback_send,
            delay,
        )),
    }
}

pub async fn block_dissemination_thread(
    id: NodeId,
    config: ChainConfig,
    peer_messenger: Arc<PeerMessenger>,
    threshold_signature: Arc<dyn ThresholdSignature>,
    mut new_block_recv: mpsc::Receiver<(NodeId, Vec<(Arc<Block>, Arc<BlkCtx>)>)>,
    mut peer_blk_dissem_recv: mpsc::Receiver<(NodeId, Vec<u8>)>,
    block_ready_send: mpsc::Sender<(NodeId, Vec<(Arc<Block>, Arc<BlkCtx>)>)>,
    pmaker_feedback_send: mpsc::Sender<Vec<u8>>,
    core_group: Arc<VCoreGroup>,
    monitor: TaskMonitor,
) {
    pf_info!(id; "block dissemination stage starting...");

    let delay = Arc::new(DelayPool::new());
    let mut insert_delay_time = Instant::now() + BLK_DISS_DELAY_INTERVAL;

    let mut block_dissemination_stage = get_block_dissemination(
        id,
        config,
        peer_messenger,
        threshold_signature,
        pmaker_feedback_send,
        delay,
    );

    let mut blks_recv = 0usize;
    let mut txns_recv = 0usize;
    let mut blks_sent = 0usize;
    let mut txns_sent = 0usize;

    let mut report_timer = get_report_timer();
    let mut task_interval = monitor.intervals();

    loop {
        tokio::select! {
            new_blk = new_block_recv.recv() => {
                // for serializing blocks sent
                let (src, new_tail) = match new_blk {
                    Some(blk) => blk,
                    None => {
                        pf_error!(id; "new_block pipe closed unexpectedly");
                        return;
                    }
                };

                blks_recv += new_tail.len();
                txns_recv += new_tail.iter().map(|(blk, _)| blk.txns.len()).sum::<usize>();

                let _permit = match core_group.acquire().await {
                    Ok(permit) => permit,
                    Err(e) => {
                        pf_error!(id; "failed to acquire allowed concurrency: {:?}", e);
                        continue;
                    }
                };

                pf_debug!(id; "got new tail {:?}", new_tail);

                if let Err(e) = block_dissemination_stage.disseminate(src, new_tail).await {
                    pf_error!(id; "failed to disseminate new block: {:?}", e);
                    continue;
                }

                drop(_permit);
            },

            wait_result = block_dissemination_stage.wait_disseminated() => {
                if let Err(e) = wait_result {
                    pf_error!(id; "failed to wait for disseminated block: {:?}", e);
                    continue;
                }

                match block_dissemination_stage.get_disseminated().await {
                    Ok((src, blks)) => {

                        blks_sent += blks.len();
                        txns_sent += blks.iter().map(|(blk, _)| blk.txns.len()).sum::<usize>();

                        if let Err(e) = block_ready_send.send((src, blks)).await {
                            pf_error!(id; "failed to send disseminated block: {:?}", e);
                            continue
                        }
                    },
                    Err(e) => {
                        pf_error!(id; "failed to get disseminated block: {:?}", e);
                        continue;
                    }
                }
            }

            peer_msg = peer_blk_dissem_recv.recv() => {
                let (src, msg) = match peer_msg {
                    Some(content) => content,
                    None => {
                        pf_error!(id; "peer_blk_dissem pipe closed unexpectedly");
                        continue;
                    }
                };

                if let Err(e) = block_dissemination_stage.handle_peer_msg(src, msg).await {
                    pf_error!(id; "failed to handle peer block dissem message: {:?}", e);
                    continue;
                }
            }

            _ = pass(), if Instant::now() > insert_delay_time => {
                // insert delay as appropriate
                tokio::task::yield_now().await;
                insert_delay_time = Instant::now() + BLK_DISS_DELAY_INTERVAL;
            }

            report_val = report_timer.changed() => {
                if let Err(e) = report_val {
                    pf_error!(id; "Waiting for report timeout failed: {}", e);
                }

                pf_info!(id; "In the last minute: blks_recv: {}, txns_recv: {}, blks_sent: {}, txns_sent: {}", blks_recv, txns_recv, blks_sent, txns_sent);
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
