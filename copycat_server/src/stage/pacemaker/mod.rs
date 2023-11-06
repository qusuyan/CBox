mod dummy;
use dummy::DummyPacemaker;

use crate::peers::PeerMessenger;
use copycat_utils::{CopycatError, NodeId};

use async_trait::async_trait;

use tokio::sync::mpsc;

use std::sync::Arc;

#[async_trait]
pub trait Pacemaker: Sync + Send {
    async fn wait_to_propose(&self) -> Result<Arc<Vec<u8>>, CopycatError>;
}

pub enum PacemakerType {
    Dummy,
}

fn get_pacemaker<TxnType, BlockType>(
    pacemaker_type: PacemakerType,
    peer_messenger: Arc<PeerMessenger<TxnType, BlockType>>,
    mut peer_pmaker_recv: mpsc::UnboundedReceiver<(NodeId, Arc<Vec<u8>>)>,
) -> Box<dyn Pacemaker> {
    match pacemaker_type {
        PacemakerType::Dummy => Box::new(DummyPacemaker {}),
        _ => todo!(),
    }
}

pub async fn pacemaker_thread<TxnType, BlockType>(
    id: NodeId,
    pacemaker_type: PacemakerType,
    peer_messenger: Arc<PeerMessenger<TxnType, BlockType>>,
    peer_pmaker_recv: mpsc::UnboundedReceiver<(NodeId, Arc<Vec<u8>>)>,
    should_propose_send: mpsc::Sender<Arc<Vec<u8>>>,
) {
    let pmaker = get_pacemaker(pacemaker_type, peer_messenger, peer_pmaker_recv);

    loop {
        let propose_msg = match pmaker.wait_to_propose().await {
            Ok(msg) => msg,
            Err(e) => {
                log::error!("Node {id}: error waiting to propose: {e:?}");
                continue;
            }
        };

        if let Err(e) = should_propose_send.send(propose_msg).await {
            log::error!("Node {id}: failed to send to should_propose pipe: {e:?}");
            continue;
        }
    }
}
