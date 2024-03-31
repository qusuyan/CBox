mod broadcast;
use broadcast::BroadcastTxnDissemination;

mod gossip;
use gossip::GossipTxnDissemination;

use crate::protocol::transaction::Txn;
use crate::protocol::DissemPattern;
use crate::utils::{CopycatError, NodeId};
use crate::{config::Config, peers::PeerMessenger};

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::mpsc;

#[async_trait]
pub trait TxnDissemination: Send + Sync {
    async fn disseminate(&self, src: NodeId, txn: &Txn) -> Result<(), CopycatError>;
}

fn get_txn_dissemination(
    id: NodeId,
    dissem_pattern: DissemPattern,
    _config: Config,
    peer_messenger: Arc<PeerMessenger>,
) -> Box<dyn TxnDissemination> {
    match dissem_pattern {
        DissemPattern::Broadcast => Box::new(BroadcastTxnDissemination::new(id, peer_messenger)),
        DissemPattern::Gossip => Box::new(GossipTxnDissemination::new(id, peer_messenger)),
        DissemPattern::Sample => Box::new(GossipTxnDissemination::new(id, peer_messenger)), // TODO: use gossip for now
    }
}

pub async fn txn_dissemination_thread(
    id: NodeId,
    dissem_pattern: DissemPattern,
    config: Config,
    enabled: bool,
    peer_messenger: Arc<PeerMessenger>,
    mut validated_txn_recv: mpsc::Receiver<(NodeId, Arc<Txn>)>,
    txn_ready_send: mpsc::Sender<Arc<Txn>>,
) {
    pf_info!(id; "txn dissemination stage starting...");

    let txn_dissemination_stage = get_txn_dissemination(id, dissem_pattern, config, peer_messenger);

    loop {
        let (src, txn) = match validated_txn_recv.recv().await {
            Some(txn) => txn,
            None => {
                pf_error!(id; "validated_txn pipe closed unexpectedly");
                return;
            }
        };

        pf_trace!(id; "got from {} new txn {:?}", src, txn);

        if enabled {
            if let Err(e) = txn_dissemination_stage.disseminate(src, &txn).await {
                pf_error!(id; "failed to disseminate txn: {:?}", e);
                continue;
            }
        }

        if let Err(e) = txn_ready_send.send(txn).await {
            pf_error!(id; "failed to send to txn_ready pipe: {:?}", e);
            continue;
        }
    }
}
