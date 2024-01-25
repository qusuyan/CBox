mod dummy;
use dummy::DummyBlockManagement;
mod bitcoin;
use bitcoin::BitcoinBlockManagement;

use copycat_protocol::transaction::Txn;
use copycat_protocol::{block::Block, CryptoScheme};
use copycat_utils::{CopycatError, NodeId};

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{Duration, Instant};

use crate::config::Config;

#[async_trait]
pub trait BlockManagement: Sync + Send {
    async fn record_new_txn(&mut self, txn: Arc<Txn>) -> Result<bool, CopycatError>;
    async fn prepare_new_block(&mut self) -> Result<(), CopycatError>;
    async fn wait_to_propose(&self) -> Result<(), CopycatError>;
    async fn get_new_block(&mut self) -> Result<Arc<Block>, CopycatError>;
    async fn validate_block(&mut self, block: Arc<Block>) -> Result<Vec<Arc<Block>>, CopycatError>;
    async fn handle_pmaker_msg(&mut self, msg: Arc<Vec<u8>>) -> Result<(), CopycatError>;
}

fn get_blk_creation(config: Config, crypto_scheme: CryptoScheme) -> Box<dyn BlockManagement> {
    match config {
        Config::Dummy => Box::new(DummyBlockManagement::new()),
        Config::Bitcoin { config } => Box::new(BitcoinBlockManagement::new(crypto_scheme, config)),
    }
}

pub async fn block_management_thread(
    id: NodeId,
    config: Config,
    crypto_scheme: CryptoScheme,
    mut peer_blk_recv: mpsc::Receiver<(NodeId, Arc<Block>)>,
    mut txn_ready_recv: mpsc::Receiver<Arc<Txn>>,
    mut pacemaker_recv: mpsc::Receiver<Arc<Vec<u8>>>,
    new_block_send: mpsc::Sender<(NodeId, Vec<Arc<Block>>)>,
) {
    log::info!("block management stage starting...");

    let mut block_management_stage = get_blk_creation(config, crypto_scheme);
    let mut batch_prepare_time = Instant::now() + Duration::from_millis(100);

    loop {
        tokio::select! {
            new_txn = txn_ready_recv.recv() => {
                match new_txn {
                    Some(txn) => {
                        log::trace!("got new txn {txn:?}");
                        if let Err(e) = block_management_stage.record_new_txn(txn).await {
                            log::error!("failed to record new txn: {e:?}");
                            continue;
                        }
                    },
                    None => {
                        log::error!("txn_ready pipe closed unexpectedly");
                        return;
                    }
                }
            },

            _ = tokio::time::sleep_until(batch_prepare_time) => {
                if let Err(e) = block_management_stage.prepare_new_block().await {
                    log::error!("failed to prepare new block: {e:?}");
                }

                batch_prepare_time = Instant::now() + Duration::from_millis(100);
            },

            wait_result = block_management_stage.wait_to_propose() => {
                if let Err(e) = wait_result {
                    log::error!("wait to propose failed: {e:?}");
                    continue;
                }

                match block_management_stage.get_new_block().await {
                    Ok(block) => {
                        log::debug!("proposing new block {block:?}");
                        if let Err(e) = new_block_send.send((id, vec![block])).await {
                            log::error!("failed to send to new_block pipe: {e:?}");
                            continue;
                        }
                    },
                    Err(e) => {
                        log::error!("error creating new block: {e:?}");
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
                        log::error!("peer_blk pipe closed unexpectedly");
                        return;
                    }
                };

                log::debug!("got from {src} new block {new_block:?}");

                match block_management_stage.validate_block(new_block.clone()).await {
                    Ok(new_tail) => {
                        if !new_tail.is_empty() {
                            if let Err(e) = new_block_send.send((src, new_tail)).await {
                                log::error!("failed to send to block_ready pipe: {e:?}");
                                continue;
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("failed to validate block: {e:?}");
                        continue;
                    }
                }
            }

            pmaker_msg = pacemaker_recv.recv() => {
                match pmaker_msg {
                    Some(msg) => {
                        if let Err(e) = block_management_stage.handle_pmaker_msg(msg).await {
                            log::error!("failed to handle pacemaker message: {e:?}");
                            continue;
                        }
                    },
                    None => {
                        log::error!("pacemaker pipe closed unexpectedly");
                        return;
                    }
                }

            }
        }
    }
}
