mod dummy;
use dummy::DummyBlockCreation;

use copycat_utils::{CopycatError, NodeId};

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::mpsc;

pub enum BlockCreationType {
    Dummy,
}

#[async_trait]
pub trait BlockCreation<TxnType, BlockType>: Sync + Send {
    async fn new_txn(&mut self, txn: Arc<TxnType>) -> Result<(), CopycatError>;
    async fn new_block(&mut self, pmaker_msg: Arc<Vec<u8>>)
        -> Result<Arc<BlockType>, CopycatError>;
}

fn get_blk_creation<TxnType, BlockType>(
    block_creation_type: BlockCreationType,
) -> Box<dyn BlockCreation<TxnType, BlockType>>
where
    DummyBlockCreation<TxnType>: BlockCreation<TxnType, BlockType>,
{
    match block_creation_type {
        BlockCreationType::Dummy => Box::new(DummyBlockCreation::new()),
    }
}

pub async fn block_creation_thread<TxnType, BlockType>(
    id: NodeId,
    block_creation_type: BlockCreationType,
    mut txn_ready_recv: mpsc::Receiver<Arc<TxnType>>,
    mut should_propose_recv: mpsc::Receiver<Arc<Vec<u8>>>,
    new_block_send: mpsc::Sender<Arc<BlockType>>,
) where
    TxnType: Sync + Send,
{
    let mut block_creation_stage = get_blk_creation(block_creation_type);

    loop {
        tokio::select! {
            new_txn = txn_ready_recv.recv() => {
                match new_txn {
                    Some(txn) => {
                        if let Err(e) = block_creation_stage.new_txn(txn).await {
                            log::error!("Node {id}: failed to record new txn: {e:?}");
                            continue;
                        }
                    },
                    None => {
                        log::error!("Node {id}: txn_ready pipe closed unexpectedly");
                        continue;
                    }
                }
            },
            should_propose = should_propose_recv.recv() => {
                match should_propose {
                    Some(pmaker_msg) => {
                        let new_blk = match block_creation_stage.new_block(pmaker_msg).await {
                            Ok(blk) => blk,
                            Err(e) => {
                                log::error!("Node {id}: failed to create new block: {e:?}");
                                continue;
                            }
                        };

                        if let Err(e) = new_block_send.send(new_blk).await {
                            log::error!("Node {id}: failed to send to new_block pipe: {e:?}");
                            continue;
                        }
                    },
                    None => {
                        log::error!("Node {id}: should_propose pipe closed unexpectedly");
                        continue;
                    }
                }
            },
        }
    }
}
