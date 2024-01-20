mod dummy;
use dummy::DummyPacemaker;

use crate::{config::Config, peers::PeerMessenger};
use copycat_utils::{CopycatError, NodeId};

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::mpsc;

#[async_trait]
pub trait Pacemaker: Sync + Send {
    async fn wait_to_propose(&self) -> Result<Arc<Vec<u8>>, CopycatError>;
}

fn get_pacemaker(
    config: Config,
    peer_messenger: Arc<PeerMessenger>,
    mut peer_pmaker_recv: mpsc::Receiver<(NodeId, Arc<Vec<u8>>)>,
) -> Box<dyn Pacemaker> {
    match config {
        Config::Dummy => Box::new(DummyPacemaker {}),
        Config::Bitcoin { .. } => Box::new(DummyPacemaker {}), // TODO
    }
}

pub async fn pacemaker_thread(
    config: Config,
    peer_messenger: Arc<PeerMessenger>,
    peer_pmaker_recv: mpsc::Receiver<(NodeId, Arc<Vec<u8>>)>,
    should_propose_send: mpsc::Sender<Arc<Vec<u8>>>,
) {
    log::info!("pacemaker starting...");

    let pmaker = get_pacemaker(config, peer_messenger, peer_pmaker_recv);

    loop {
        let propose_msg = match pmaker.wait_to_propose().await {
            Ok(msg) => msg,
            Err(e) => {
                log::error!("error waiting to propose: {e:?}");
                continue;
            }
        };

        log::debug!("got pacemaker peer message {propose_msg:?}");

        if let Err(e) = should_propose_send.send(propose_msg).await {
            log::error!("failed to send to should_propose pipe: {e:?}");
            continue;
        }
    }
}
