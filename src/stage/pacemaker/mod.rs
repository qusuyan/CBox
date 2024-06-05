mod dummy;
use dummy::DummyPacemaker;

mod avalanche;
use avalanche::AvalanchePacemaker;

use crate::utils::{CopycatError, NodeId};
use crate::{config::Config, peers::PeerMessenger};

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{Duration, Instant};

use atomic_float::AtomicF64;
use std::sync::atomic::Ordering;

#[async_trait]
pub trait Pacemaker: Sync + Send {
    async fn wait_to_propose(&self);
    async fn get_propose_msg(&mut self) -> Result<Vec<u8>, CopycatError>;
    async fn handle_feedback(&mut self, feedback: Vec<u8>) -> Result<(), CopycatError>;
    async fn handle_peer_msg(&mut self, src: NodeId, msg: Vec<u8>) -> Result<(), CopycatError>;
}

fn get_pacemaker(
    _id: NodeId,
    config: Config,
    peer_messenger: Arc<PeerMessenger>,
) -> Box<dyn Pacemaker> {
    match config {
        Config::Dummy => Box::new(DummyPacemaker::new()),
        Config::Bitcoin { .. } => Box::new(DummyPacemaker::new()), // TODO
        Config::Avalanche { config } => Box::new(AvalanchePacemaker::new(config)), // TODO
    }
}

pub async fn pacemaker_thread(
    id: NodeId,
    config: Config,
    peer_messenger: Arc<PeerMessenger>,
    mut peer_pmaker_recv: mpsc::Receiver<(NodeId, Vec<u8>)>,
    mut pmaker_feedback_recv: mpsc::Receiver<Vec<u8>>,
    should_propose_send: mpsc::Sender<Vec<u8>>,
) {
    pf_info!(id; "pacemaker starting...");

    let delay = Arc::new(AtomicF64::new(0f64));

    let mut pmaker = get_pacemaker(id, config, peer_messenger);

    let mut report_timeout = Instant::now() + Duration::from_secs(60);

    loop {
        tokio::select! {
            _ = pmaker.wait_to_propose() => {
                let propose_msg = match pmaker.get_propose_msg().await {
                    Ok(msg) => msg,
                    Err(e) => {
                        pf_error!(id; "error waiting to propose: {:?}", e);
                        continue;
                    }
                };

                pf_debug!(id; "sending propose msg {:?}", propose_msg);

                if let Err(e) = should_propose_send.send(propose_msg).await {
                    pf_error!(id; "failed to send to should_propose pipe: {:?}", e);
                    continue;
                }
            },
            feedback_msg = pmaker_feedback_recv.recv() => {
                let feedback = match feedback_msg {
                    Some(msg) => msg,
                    None => {
                        pf_error!(id; "pmaker_feedback pipe closed unexpectedly");
                        return;
                    }
                };

                pf_debug!(id; "got feedback from decide stage");
                if let Err(e) = pmaker.handle_feedback(feedback).await {
                    pf_error!(id; "failed to handle feedback: {:?}", e);
                    continue;
                }
            }
            peer_msg = peer_pmaker_recv.recv() => {
                let (src, msg) = match peer_msg {
                    Some(msg) => msg,
                    None => {
                        pf_error!(id; "peer_pmaker pipe closed unexpectedly");
                        return;
                    }
                };

                pf_debug!(id; "got peer pmaker msg from {}", src);
                if let Err(e) = pmaker.handle_peer_msg(src, msg).await {
                    pf_error!(id; "failed to handle peer msg: {:?}", e);
                    continue;
                }
            }
            _ = tokio::time::sleep_until(report_timeout) => {
                // reset report time
                report_timeout = Instant::now() + Duration::from_secs(60);
            }
        }

        // insert delay as appropriate
        let sleep_time = delay.load(Ordering::Relaxed);
        if sleep_time > 0.05 {
            tokio::time::sleep(Duration::from_secs_f64(sleep_time)).await;
            delay.store(0f64, Ordering::Relaxed);
        }
    }
}
