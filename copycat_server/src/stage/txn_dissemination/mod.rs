mod broadcast;
use broadcast::BroadcastTxnDissemination;

use crate::peers::PeerMessenger;
use copycat_protocol::transaction::Txn;
use copycat_protocol::ChainType;
use copycat_utils::{CopycatError, NodeId};

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::mpsc;

#[async_trait]
pub trait TxnDissemination: Send + Sync {
    async fn disseminate(&self, txn: &Txn) -> Result<(), CopycatError>;
}

fn get_txn_dissemination(
    chain_type: ChainType,
    peer_messenger: Arc<PeerMessenger>,
) -> Box<dyn TxnDissemination> {
    match chain_type {
        ChainType::Dummy => Box::new(BroadcastTxnDissemination::new(peer_messenger)),
        ChainType::Bitcoin => todo!(),
    }
}

pub async fn txn_dissemination_thread(
    id: NodeId,
    chain_type: ChainType,
    peer_messenger: Arc<PeerMessenger>,
    mut validated_txn_recv: mpsc::Receiver<(Arc<Txn>, bool)>,
    txn_ready_send: mpsc::Sender<Arc<Txn>>,
) {
    log::trace!("Node {id}: Txn Dissemination stage starting...");

    let txn_dissemination_stage = get_txn_dissemination(chain_type, peer_messenger);

    loop {
        let (txn, should_disseminate) = match validated_txn_recv.recv().await {
            Some(txn) => txn,
            None => {
                log::error!("Node {id}: validated_txn pipe closed unexpectedly");
                continue;
            }
        };

        log::trace!("Node {id}: got new txn {txn:?}, should_disseminate = {should_disseminate}");

        if should_disseminate {
            if let Err(e) = txn_dissemination_stage.disseminate(&txn).await {
                log::error!("Node {id}: failed to disseminate txn: {e:?}");
                continue;
            }
        }

        if let Err(e) = txn_ready_send.send(txn).await {
            log::error!("Node {id}: failed to send to txn_ready pipe: {e:?}");
            continue;
        }
    }
}
