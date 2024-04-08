mod dummy;
use dummy::DummyBlockManagement;
mod bitcoin;
use bitcoin::BitcoinBlockManagement;
mod avalanche;

use crate::protocol::block::Block;
use crate::protocol::crypto::Hash;
use crate::protocol::transaction::Txn;
use crate::protocol::CryptoScheme;
use crate::utils::{CopycatError, NodeId};

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{Duration, Instant};

use crate::config::Config;
use crate::peers::PeerMessenger;

use self::avalanche::AvalancheBlockManagement;

#[async_trait]
pub trait BlockManagement: Sync + Send {
    async fn record_new_txn(&mut self, txn: Arc<Txn>) -> Result<bool, CopycatError>;
    async fn prepare_new_block(&mut self) -> Result<(), CopycatError>;
    async fn wait_to_propose(&self) -> Result<(), CopycatError>;
    async fn get_new_block(&mut self) -> Result<Arc<Block>, CopycatError>;
    async fn validate_block(&mut self, block: Arc<Block>) -> Result<Vec<Arc<Block>>, CopycatError>;
    async fn handle_pmaker_msg(&mut self, msg: Arc<Vec<u8>>) -> Result<(), CopycatError>;
    async fn handle_peer_blk_req(&mut self, peer: NodeId, blk_id: Hash)
        -> Result<(), CopycatError>;
    // async fn handle_peer_blk_resp(
    //     &mut self,
    //     peer: NodeId,
    //     blk_id: Hash,
    //     block: Arc<Block>,
    // ) -> Result<(), CopycatError>;
}

fn get_blk_creation(
    id: NodeId,
    config: Config,
    crypto_scheme: CryptoScheme,
    peer_messenger: Arc<PeerMessenger>,
) -> Box<dyn BlockManagement> {
    match config {
        Config::Dummy => Box::new(DummyBlockManagement::new()),
        Config::Bitcoin { config } => Box::new(BitcoinBlockManagement::new(
            id,
            crypto_scheme,
            config,
            peer_messenger,
        )),
        Config::Avalanche { config } => {
            Box::new(AvalancheBlockManagement::new(id, crypto_scheme, config))
        }
    }
}

pub async fn block_management_thread(
    id: NodeId,
    config: Config,
    crypto_scheme: CryptoScheme,
    mut peer_blk_recv: mpsc::Receiver<(NodeId, Arc<Block>)>,
    mut peer_blk_req_recv: mpsc::Receiver<(NodeId, Hash)>,
    // mut peer_blk_resp_recv: mpsc::Receiver<(NodeId, (Hash, Arc<Block>))>,
    peer_messenger: Arc<PeerMessenger>,
    mut txn_ready_recv: mpsc::Receiver<Arc<Txn>>,
    mut pacemaker_recv: mpsc::Receiver<Arc<Vec<u8>>>,
    new_block_send: mpsc::Sender<(NodeId, Vec<Arc<Block>>)>,
) {
    pf_info!(id; "block management stage starting...");

    let mut block_management_stage = get_blk_creation(id, config, crypto_scheme, peer_messenger);
    let mut batch_prepare_time = Instant::now() + Duration::from_millis(100);

    let mut report_timeout = Instant::now() + Duration::from_secs(60);
    let mut self_txns_sent = 0;
    let mut self_blks_sent = 0;
    let mut peer_txns_sent = 0;
    let mut peer_blks_sent = 0;
    let mut txns_recv = 0;

    loop {
        tokio::select! {
            new_txn = txn_ready_recv.recv() => {
                match new_txn {
                    Some(txn) => {
                        pf_trace!(id; "got new txn {:?}", txn);
                        txns_recv += 1;
                        if let Err(e) = block_management_stage.record_new_txn(txn).await {
                            pf_error!(id; "failed to record new txn: {:?}", e);
                            continue;
                        }
                    },
                    None => {
                        pf_error!(id; "txn_ready pipe closed unexpectedly");
                        return;
                    }
                }
            },

            _ = tokio::time::sleep_until(batch_prepare_time) => {
                if let Err(e) = block_management_stage.prepare_new_block().await {
                    pf_error!(id; "failed to prepare new block: {:?}", e);
                }

                batch_prepare_time = Instant::now() + Duration::from_millis(100);
            },

            wait_result = block_management_stage.wait_to_propose() => {
                if let Err(e) = wait_result {
                    pf_error!(id; "wait to propose failed: {:?}", e);
                    continue;
                }

                match block_management_stage.get_new_block().await {
                    Ok(block) => {
                        pf_debug!(id; "proposing new block {:?}", block);
                        self_blks_sent += 1;
                        self_txns_sent += block.txns.len();
                        if let Err(e) = new_block_send.send((id, vec![block])).await {
                            pf_error!(id; "failed to send to new_block pipe: {:?}", e);
                            continue;
                        }
                    },
                    Err(e) => {
                        pf_error!(id; "error creating new block: {:?}", e);
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
                        pf_error!(id; "peer_blk pipe closed unexpectedly");
                        return;
                    }
                };

                pf_debug!(id; "got from {} new block {:?}", src, new_block);

                match block_management_stage.validate_block(new_block.clone()).await {
                    Ok(new_tail) => {
                        if !new_tail.is_empty() {
                            for blk in new_tail.iter() {
                                peer_blks_sent += 1;
                                peer_txns_sent += blk.txns.len();
                            }
                            if let Err(e) = new_block_send.send((src, new_tail)).await {
                                pf_error!(id; "failed to send to block_ready pipe: {:?}", e);
                                continue;
                            }
                        }
                    }
                    Err(e) => {
                        pf_error!(id; "failed to validate block: {:?}", e);
                        continue;
                    }
                }
            }

            pmaker_msg = pacemaker_recv.recv() => {
                match pmaker_msg {
                    Some(msg) => {
                        if let Err(e) = block_management_stage.handle_pmaker_msg(msg).await {
                            pf_error!(id; "failed to handle pacemaker message: {:?}", e);
                            continue;
                        }
                    },
                    None => {
                        pf_error!(id; "pacemaker pipe closed unexpectedly");
                        return;
                    }
                }
            }

            req = peer_blk_req_recv.recv() => {
                match req {
                    Some((peer, blk_id)) => {
                        pf_debug!(id; "got block request for {}", blk_id);
                        if let Err(e) = block_management_stage.handle_peer_blk_req(peer, blk_id).await {
                            pf_error!(id; "error handling peer block request: {:?}", e);
                            continue;
                        }
                    }
                    None => {
                        pf_error!(id; "peer_blk_req pipe closed unexpectedly");
                        continue;
                    }
                }
            }

            _ = tokio::time::sleep_until(report_timeout) => {
                pf_info!(id; "In the last minute: txns_recv: {}, self_blks_sent: {}, self_txns_sent: {}, peer_blks_sent: {}, peer_txns_sent: {}", txns_recv, self_blks_sent, self_txns_sent, peer_blks_sent, peer_txns_sent);
                txns_recv = 0;
                self_blks_sent = 0;
                self_txns_sent = 0;
                peer_blks_sent = 0;
                peer_txns_sent = 0;
                report_timeout = Instant::now() + Duration::from_secs(60);
            }

            // resp = peer_blk_resp_recv.recv() => {
            //     match resp {
            //         Some((src, (id, block))) => {
            //             if let Err(e) = block_management_stage.handle_peer_blk_resp(src, id, block).await {
            //                 pf_error!(id; "error handling peer block response: {:?}", e);
            //                 continue;
            //             }
            //         }
            //         None => {
            //             pf_error!(id; "peer_blk_resp pipe closed unexpectedly");
            //             continue;
            //         }
            //     }
            // }
        }
    }
}
