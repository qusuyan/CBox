mod dummy;
use dummy::DummyBlockManagement;
mod bitcoin;

use copycat_protocol::block::Block;
use copycat_protocol::transaction::Txn;
use copycat_protocol::ChainType;
use copycat_utils::{CopycatError, NodeId};

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::mpsc;

#[async_trait]
pub trait BlockManagement: Sync + Send {
    async fn record_new_txn(&mut self, txn: Arc<Txn>) -> Result<(), CopycatError>;
    async fn prepare_new_block(&mut self) -> Result<(), CopycatError>;
    async fn wait_to_propose(&mut self) -> Result<(), CopycatError>;
    async fn get_new_block(&mut self) -> Result<Arc<Block>, CopycatError>;
    async fn validate_block(&mut self, block: &Block) -> Result<bool, CopycatError>;
    async fn handle_pmaker_msg(&mut self, msg: Arc<Vec<u8>>) -> Result<(), CopycatError>;
}

fn get_blk_creation(chain_type: ChainType) -> Box<dyn BlockManagement> {
    match chain_type {
        ChainType::Dummy => Box::new(DummyBlockManagement::new()),
        ChainType::Bitcoin => todo!(),
    }
}

pub async fn block_management_thread(
    id: NodeId,
    chain_type: ChainType,
    mut peer_blk_recv: mpsc::UnboundedReceiver<(NodeId, Arc<Block>)>,
    mut txn_ready_recv: mpsc::Receiver<Arc<Txn>>,
    mut pacemaker_recv: mpsc::Receiver<Arc<Vec<u8>>>,
    new_block_send: mpsc::Sender<Arc<Block>>,
) {
    log::trace!("Node {id}: Txn Validation stage starting...");

    let mut block_management_stage = get_blk_creation(chain_type);

    loop {
        // first try to construct / add-on to block to be proposed
        if let Err(e) = block_management_stage.prepare_new_block().await {
            log::error!("Node {id}: failed to prepare new block: {e:?}");
        }

        tokio::select! {
            new_txn = txn_ready_recv.recv() => {
                match new_txn {
                    Some(txn) => {
                        log::trace!("Node {id}: got new txn {txn:?}");
                        if let Err(e) = block_management_stage.record_new_txn(txn).await {
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

            wait_result = block_management_stage.wait_to_propose() => {
                if let Err(e) = wait_result {
                    log::error!("Node {id}: wait to propose failed: {e:?}");
                    continue;
                }

                match block_management_stage.get_new_block().await {
                    Ok(block) => {
                        log::trace!("Node {id}: proposing new block {block:?}");
                        if let Err(e) = new_block_send.send(block).await {
                            log::error!("Node {id}: failed to send to new_block pipe: {e:?}");
                            continue;
                        }
                    },
                    Err(e) => {
                        log::error!("Node {id}: error creating new block: {e:?}");
                        continue;
                    }
                }
            },

            peer_blk = peer_blk_recv.recv() => {
                let (src, new_block) = match peer_blk {
                    Some((src, blk)) => {
                        if src == id {
                            // ignore blocks proposed by myself
                            continue;
                        }
                        (src, blk)
                    }
                    None => {
                        log::error!("Node {id}: peer_blk pipe closed unexpectedly");
                        continue;
                    }
                };

                log::trace!("Node {id}: got from {src} new block {new_block:?}");

                match block_management_stage.validate_block(&new_block).await {
                    Ok(valid) => {
                        if !valid {
                            log::warn!("Node {id}: got invalid block from {src}, ignoring...");
                            continue;
                        }

                        if let Err(e) = new_block_send.send(new_block).await {
                            log::error!("Node {id}: failed to send to block_ready pipe: {e:?}");
                            continue;
                        }
                    }
                    Err(e) => {
                        log::error!("Node {id}: failed to validate block: {e:?}");
                        continue;
                    }
                }
            }

            pmaker_msg = pacemaker_recv.recv() => {
                match pmaker_msg {
                    Some(msg) => {
                        if let Err(e) = block_management_stage.handle_pmaker_msg(msg).await {
                            log::error!("Node {id}: failed to handle pacemaker message: {e:?}");
                            continue;
                        }
                    },
                    None => {
                        log::error!("Node {id}: pacemaker pipe closed unexpectedly");
                        continue;
                    }
                }

            }
        }
    }
}
