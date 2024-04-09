mod dummy;
use dummy::DummyCommit;

use crate::protocol::transaction::Txn;
use crate::{CopycatError, NodeId};

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::mpsc;

use crate::config::Config;

#[async_trait]
pub trait Commit: Sync + Send {
    async fn commit(&self, block: &Vec<Arc<Txn>>) -> Result<(), CopycatError>;
}

pub fn get_commit(_id: NodeId, config: Config) -> Box<dyn Commit> {
    match config {
        Config::Dummy => Box::new(DummyCommit::new()),
        Config::Bitcoin { .. } => Box::new(DummyCommit::new()), // TODO:
        Config::Avalanche { .. } => Box::new(DummyCommit::new()), // TODO:
    }
}

pub async fn commit_thread(
    id: NodeId,
    config: Config,
    mut commit_recv: mpsc::UnboundedReceiver<(u64, Vec<Arc<Txn>>)>,
    executed_send: mpsc::UnboundedSender<(u64, Vec<Arc<Txn>>)>,
) {
    pf_info!(id; "commit stage starting...");

    let commit_stage = get_commit(id, config);

    loop {
        let (height, txn_batch) = match commit_recv.recv().await {
            Some(blk) => blk,
            None => {
                pf_error!(id; "commit pipe closed unexpectedly");
                return;
            }
        };

        pf_debug!(id; "got new txn batch ({})", txn_batch.len());

        if let Err(e) = commit_stage.commit(&txn_batch).await {
            pf_error!(id; "failed to commit: {:?}", e);
            continue;
        }

        if let Err(e) = executed_send.send((height, txn_batch)) {
            pf_error!(id; "failed to send committed txns: {:?}", e);
            continue;
        }
    }
}
