mod dummy;
use dummy::DummyCommit;

use crate::protocol::block::Block;
use crate::protocol::transaction::Txn;
use crate::utils::CopycatError;

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::mpsc;

use crate::config::Config;

#[async_trait]
pub trait Commit: Sync + Send {
    async fn commit(&self, block: Arc<Block>) -> Result<(), CopycatError>;
}

pub fn get_commit(config: Config, executed_send: mpsc::Sender<Arc<Txn>>) -> Box<dyn Commit> {
    match config {
        Config::Dummy => Box::new(DummyCommit::new(executed_send)),
        Config::Bitcoin { .. } => Box::new(DummyCommit::new(executed_send)), // TODO:
    }
}

pub async fn commit_thread(
    config: Config,
    mut commit_recv: mpsc::Receiver<Arc<Block>>,
    executed_send: mpsc::Sender<Arc<Txn>>,
) {
    log::info!("commit stage starting...");

    let commit_stage = get_commit(config, executed_send);

    loop {
        let block = match commit_recv.recv().await {
            Some(blk) => blk,
            None => {
                log::error!("commit pipe closed unexpectedly");
                return;
            }
        };

        log::debug!("got new block {block:?}");

        if let Err(e) = commit_stage.commit(block).await {
            log::error!("failed to commit: {e:?}");
            continue;
        }
    }
}
