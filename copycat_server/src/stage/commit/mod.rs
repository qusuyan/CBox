mod dummy;
use dummy::DummyCommit;

use copycat_protocol::block::Block;
use copycat_protocol::transaction::Txn;
use copycat_protocol::ChainType;
use copycat_utils::{CopycatError, NodeId};

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::mpsc;

#[async_trait]
pub trait Commit: Sync + Send {
    async fn commit(&self, block: Arc<Block>) -> Result<(), CopycatError>;
}

pub fn get_commit(chain_type: ChainType, executed_send: mpsc::Sender<Arc<Txn>>) -> Box<dyn Commit> {
    match chain_type {
        ChainType::Dummy => Box::new(DummyCommit::new(executed_send)),
        ChainType::Bitcoin => Box::new(DummyCommit::new(executed_send)), // TODO:
    }
}

pub async fn commit_thread(
    id: NodeId,
    chain_type: ChainType,
    mut commit_recv: mpsc::Receiver<Arc<Block>>,
    executed_send: mpsc::Sender<Arc<Txn>>,
) {
    log::info!("Node {id}: commit stage starting...");

    let commit_stage = get_commit(chain_type, executed_send);

    loop {
        let block = match commit_recv.recv().await {
            Some(blk) => blk,
            None => {
                log::error!("Node {id}: commit pipe closed unexpectedly");
                continue;
            }
        };

        log::debug!("Node {id}: got new block {block:?}");

        if let Err(e) = commit_stage.commit(block).await {
            log::error!("Node {id}: failed to commit: {e:?}");
            continue;
        }
    }
}
