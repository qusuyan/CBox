mod dummy;
use dummy::DummyPacemaker;

use crate::peers::PeerMessenger;
use copycat_protocol::ChainType;
use copycat_utils::{CopycatError, NodeId};

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::mpsc;

#[async_trait]
pub trait Pacemaker: Sync + Send {
    async fn wait_to_propose(&self) -> Result<Arc<Vec<u8>>, CopycatError>;
}

fn get_pacemaker(
    chain_type: ChainType,
    peer_messenger: Arc<PeerMessenger>,
    mut peer_pmaker_recv: mpsc::UnboundedReceiver<(NodeId, Arc<Vec<u8>>)>,
) -> Box<dyn Pacemaker> {
    match chain_type {
        ChainType::Dummy => Box::new(DummyPacemaker {}),
        ChainType::Bitcoin => todo!(),
    }
}

pub async fn pacemaker_thread(
    id: NodeId,
    chain_type: ChainType,
    peer_messenger: Arc<PeerMessenger>,
    peer_pmaker_recv: mpsc::UnboundedReceiver<(NodeId, Arc<Vec<u8>>)>,
    should_propose_send: mpsc::Sender<Arc<Vec<u8>>>,
) {
    log::trace!("Node {id}: pacemaker starting...");

    let pmaker = get_pacemaker(chain_type, peer_messenger, peer_pmaker_recv);

    loop {
        let propose_msg = match pmaker.wait_to_propose().await {
            Ok(msg) => msg,
            Err(e) => {
                log::error!("Node {id}: error waiting to propose: {e:?}");
                continue;
            }
        };

        log::trace!("Node {id}: got pacemaker peer message {propose_msg:?}");

        if let Err(e) = should_propose_send.send(propose_msg).await {
            log::error!("Node {id}: failed to send to should_propose pipe: {e:?}");
            continue;
        }
    }
}
