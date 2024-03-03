mod dummy;
use dummy::DummyCommit;

use crate::protocol::block::Block;
use crate::protocol::transaction::Txn;
use crate::{CopycatError, NodeId};

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::mpsc;

use crate::config::Config;

#[async_trait]
pub trait Commit: Sync + Send {
    async fn commit(&self, block: Arc<Block>) -> Result<(), CopycatError>;
}

pub fn get_commit(_id: NodeId, config: Config) -> Box<dyn Commit> {
    match config {
        Config::Dummy => Box::new(DummyCommit::new()),
        Config::Bitcoin { .. } => Box::new(DummyCommit::new()), // TODO:
    }
}

pub async fn commit_thread(
    id: NodeId,
    config: Config,
    mut commit_recv: mpsc::Receiver<(u64, Arc<Block>)>,
    executed_send: mpsc::Sender<(u64, Vec<Arc<Txn>>)>,
) {
    pf_info!(id; "commit stage starting...");

    let commit_stage = get_commit(id, config);

    loop {
        let (height, block) = match commit_recv.recv().await {
            Some(blk) => blk,
            None => {
                pf_error!(id; "commit pipe closed unexpectedly");
                return;
            }
        };

        pf_debug!(id; "got new block {:?}", block);

        if let Err(e) = commit_stage.commit(block.clone()).await {
            pf_error!(id; "failed to commit: {:?}", e);
            continue;
        }

        if let Err(e) = executed_send.send((height, block.txns.clone())).await {
            pf_error!(id; "failed to send committed txns: {:?}", e);
            continue;
        }
    }
}
