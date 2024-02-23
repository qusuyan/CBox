mod broadcast;
use broadcast::BroadcastTxnDissemination;

mod gossip;
use gossip::GossipTxnDissemination;

use crate::{config::Config, peers::PeerMessenger};
use copycat_protocol::transaction::Txn;
use copycat_protocol::DissemPattern;
use copycat_utils::{CopycatError, NodeId};

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
    log::info!("txn dissemination stage starting...");

    let txn_dissemination_stage = get_txn_dissemination(id, dissem_pattern, config, peer_messenger);

    loop {
        let (src, txn) = match validated_txn_recv.recv().await {
            Some(txn) => txn,
            None => {
                log::error!("validated_txn pipe closed unexpectedly");
                return;
            }
        };

        log::trace!("got from {src} new txn {txn:?}");

        if enabled {
            if let Err(e) = txn_dissemination_stage.disseminate(src, &txn).await {
                log::error!("failed to disseminate txn: {e:?}");
                continue;
            }
        }

        if let Err(e) = txn_ready_send.send(txn).await {
            log::error!("failed to send to txn_ready pipe: {e:?}");
            continue;
        }
    }
}
