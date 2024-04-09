mod dummy;
use dummy::DummyPacemaker;

use crate::utils::{CopycatError, NodeId};
use crate::{config::Config, peers::PeerMessenger};

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::mpsc;

#[async_trait]
pub trait Pacemaker: Sync + Send {
    async fn wait_to_propose(&self) -> Result<Arc<Vec<u8>>, CopycatError>;
}

fn get_pacemaker(
    _id: NodeId,
    config: Config,
    peer_messenger: Arc<PeerMessenger>,
    mut peer_pmaker_recv: mpsc::UnboundedReceiver<(NodeId, Arc<Vec<u8>>)>,
) -> Box<dyn Pacemaker> {
    match config {
        Config::Dummy => Box::new(DummyPacemaker {}),
        Config::Bitcoin { .. } => Box::new(DummyPacemaker {}), // TODO
        Config::Avalanche { .. } => Box::new(DummyPacemaker {}), // TODO
    }
}

pub async fn pacemaker_thread(
    id: NodeId,
    config: Config,
    peer_messenger: Arc<PeerMessenger>,
    peer_pmaker_recv: mpsc::UnboundedReceiver<(NodeId, Arc<Vec<u8>>)>,
    should_propose_send: mpsc::UnboundedSender<Arc<Vec<u8>>>,
) {
    pf_info!(id; "pacemaker starting...");

    let pmaker = get_pacemaker(id, config, peer_messenger, peer_pmaker_recv);

    loop {
        let propose_msg = match pmaker.wait_to_propose().await {
            Ok(msg) => msg,
            Err(e) => {
                pf_error!(id; "error waiting to propose: {:?}", e);
                continue;
            }
        };

        pf_debug!(id; "got pacemaker peer message {:?}", propose_msg);

        if let Err(e) = should_propose_send.send(propose_msg) {
            pf_error!(id; "failed to send to should_propose pipe: {:?}", e);
            continue;
        }
    }
}
