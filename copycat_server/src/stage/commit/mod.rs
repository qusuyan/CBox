mod dummy;
use dummy::DummyCommit;

use async_trait::async_trait;

use crate::chain::BlockTrait;
use copycat_utils::{CopycatError, NodeId};

use std::sync::Arc;
use tokio::sync::mpsc;

#[async_trait]
pub trait Commit<BlockType: ?Sized>: Sync + Send {
    async fn commit(&self, block: Arc<BlockType>) -> Result<(), CopycatError>;
}

pub enum CommitType {
    Dummy,
}

pub fn get_commit<TxnType, BlockType: ?Sized>(
    commit_type: CommitType,
    executed_send: mpsc::Sender<Arc<TxnType>>,
) -> Box<dyn Commit<BlockType>>
where
    TxnType: 'static,
    DummyCommit<TxnType>: Commit<BlockType>,
{
    match commit_type {
        CommitType::Dummy => Box::new(DummyCommit::new(executed_send)),
    }
}

pub async fn commit_thread<TxnType, BlockType>(
    id: NodeId,
    commit_type: CommitType,
    mut commit_recv: mpsc::Receiver<Arc<BlockType>>,
    executed_send: mpsc::Sender<Arc<TxnType>>,
) where
    TxnType: 'static + Sync + Send,
    BlockType: 'static + BlockTrait<TxnType>,
{
    let commit_stage = get_commit(commit_type, executed_send);

    loop {
        let block = match commit_recv.recv().await {
            Some(blk) => blk,
            None => {
                log::error!("Node {id}: commit pipe closed unexpectedly");
                continue;
            }
        };

        if let Err(e) = commit_stage.commit(block).await {
            log::error!("Node {id}: failed to commit: {e:?}");
            continue;
        }
    }
}
