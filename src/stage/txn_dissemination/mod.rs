mod broadcast;
use broadcast::BroadcastTxnDissemination;

mod gossip;
use get_size::GetSize;
use gossip::GossipTxnDissemination;

use crate::protocol::transaction::Txn;
use crate::protocol::DissemPattern;
use crate::utils::{CopycatError, NodeId};
use crate::{config::Config, peers::PeerMessenger};

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{Duration, Instant};

#[async_trait]
pub trait TxnDissemination: Send + Sync {
    async fn disseminate(&self, txn_batch: &Vec<(u64, Arc<Txn>)>) -> Result<(), CopycatError>;
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
    let mut batch = vec![];
    let mut txn_dissem_time = Instant::now() + Duration::from_millis(100);

    async fn wait_send_batch(batch: &Vec<(u64, Arc<Txn>)>, timeout: Instant) {
        let notify = tokio::sync::Notify::new();
        if batch.len() == 0 {
            notify.notified().await;
        } else if batch.get_size() > 0x100000 {
            return;
        }
        tokio::time::sleep_until(timeout).await;
    }

    loop {
        tokio::select! {
            new_txn = validated_txn_recv.recv() => {
                let (src, txn) = match new_txn {
                    Some(txn) => txn,
                    None => {
                        pf_error!(id; "validated_txn pipe closed unexpectedly");
                        return;
                    }
                };

                pf_trace!(id; "got from {} new txn {:?}", src, txn);

                batch.push((src, txn))
            }

            _ = wait_send_batch(&batch, txn_dissem_time) => {
                let send_batch = batch.drain(0..).collect();
                if enabled {
                    if let Err(e) = txn_dissemination_stage.disseminate(&send_batch).await {
                        pf_error!(id; "failed to disseminate txn: {:?}", e);
                        continue;
                    }
                }

                for (_, txn) in send_batch.into_iter() {
                    if let Err(e) = txn_ready_send.send(txn).await {
                        pf_error!(id; "failed to send to txn_ready pipe: {:?}", e);
                        continue;
                    }
                }

                txn_dissem_time = Instant::now() + Duration::from_millis(100);
            }
        }
    }
}
