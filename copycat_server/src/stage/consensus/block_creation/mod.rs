use copycat_utils::{CopycatError, NodeId};

use async_trait::async_trait;

use tokio::sync::mpsc;

pub enum BlockCreationType {}

#[async_trait]
pub trait BlockCreation<TxnType, BlockType> {
    async fn new_txn(&self, txn: TxnType) -> Result<(), CopycatError>;
    async fn new_block(&self, pmaker_msg: Vec<u8>) -> Result<BlockType, CopycatError>;
}

fn get_blk_creation<TxnType, BlockType>(
    block_creation_type: BlockCreationType,
) -> Box<dyn BlockCreation<TxnType, BlockType>> {
    todo!();
}

pub async fn block_creation_thread<TxnType, BlockType>(
    id: NodeId,
    block_creation_type: BlockCreationType,
    mut txn_ready_recv: mpsc::Receiver<TxnType>,
    mut should_propose_recv: mpsc::Receiver<Vec<u8>>,
    new_block_send: mpsc::Sender<BlockType>,
) {
    let block_creation_stage = get_blk_creation(block_creation_type);

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
