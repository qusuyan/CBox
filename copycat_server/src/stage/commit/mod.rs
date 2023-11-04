use copycat_utils::{CopycatError, NodeId};

use async_trait::async_trait;

use tokio::sync::mpsc;

#[async_trait]
pub trait Commit<BlockType> {
    async fn commit(&self, block: BlockType) -> Result<(), CopycatError>;
}

pub enum CommitType {}

pub fn get_commit<BlockType>(commit_type: CommitType) -> Box<dyn Commit<BlockType>> {
    todo!();
}

pub async fn commit_thread<BlockType>(
    id: NodeId,
    commit_type: CommitType,
    mut commit_recv: mpsc::Receiver<BlockType>,
) {
    let commit_stage = get_commit(commit_type);

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
