use crate::peers::PeerMessenger;
use copycat_utils::{CopycatError, NodeId};

use async_trait::async_trait;

use tokio::sync::mpsc;

#[async_trait]
pub trait Pacemaker {
    async fn wait_to_propose(&self) -> Result<Vec<u8>, CopycatError>;
}

pub enum PacemakerType {}

fn get_pacemaker<TxnType, BlockType>(
    pacemaker_type: PacemakerType,
    peer_messenger: PeerMessenger<TxnType, BlockType>,
) -> Box<dyn Pacemaker> {
    todo!();
}

pub async fn pacemaker_thread<TxnType, BlockType>(
    id: NodeId,
    pacemaker_type: PacemakerType,
    peer_messenger: PeerMessenger<TxnType, BlockType>,
    should_propose_send: mpsc::Sender<Vec<u8>>,
) {
    let pmaker = get_pacemaker(pacemaker_type, peer_messenger);

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
