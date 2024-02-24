use copycat_protocol::{block::Block, crypto::Hash, transaction::Txn, MsgType};
use copycat_utils::{CopycatError, NodeId};
use mailbox_client::{ClientStubRecvHalf, ClientStubSendHalf};

use serde::{Deserialize, Serialize};
use tokio::{sync::mpsc, task::JoinHandle};

use std::{collections::HashSet, fmt::Debug, sync::Arc};

#[derive(Debug, Serialize, Deserialize)]
enum SendRequest {
    Send { dest: NodeId, msg: MsgType },
    Broadcast { msg: MsgType },
}

pub struct PeerMessenger {
    transport_hub: ClientStubSendHalf<MsgType>,
    neighbors: HashSet<NodeId>,
    _peer_receiver_rt: tokio::runtime::Runtime,
    _peer_receiver_handle: JoinHandle<()>,
}

impl PeerMessenger {
    pub async fn new(
        id: NodeId,
        neighbors: HashSet<NodeId>,
    ) -> Result<
        (
            Self,
            mpsc::Receiver<(NodeId, Arc<Txn>)>,
            mpsc::Receiver<(NodeId, Arc<Block>)>,
            mpsc::Receiver<(NodeId, Arc<Vec<u8>>)>,
            mpsc::Receiver<(NodeId, Arc<Vec<u8>>)>,
            mpsc::Receiver<(NodeId, Hash)>,
            // mpsc::Receiver<(NodeId, (Hash, Arc<Block>))>,
        ),
        CopycatError,
    > {
        let (send_half, recv_half) = mailbox_client::new_stub(id).await?;

        let (rx_txn_send, rx_txn_recv) = mpsc::channel(0x1000000);
        let (rx_blk_send, rx_blk_recv) = mpsc::channel(0x100000);
        let (rx_consensus_send, rx_consensus_recv) = mpsc::channel(0x100000);
        let (rx_pmaker_send, rx_pmaker_recv) = mpsc::channel(0x100000);
        let (rx_blk_req_send, rx_blk_req_recv) = mpsc::channel(0x100000);
        // let (rx_blk_resp_send, rx_blk_resp_recv) = mpsc::channel(0x100000);

        // put receiver runtime on a separate thread for the bug here: https://github.com/tokio-rs/tokio/issues/4730
        let _peer_receiver_rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .thread_name(format!("copycat-server-{}-recver", id))
            .build()
            .unwrap();
        let _peer_receiver_handle = _peer_receiver_rt.spawn(Self::peer_receiver_thread(
            recv_half,
            rx_txn_send,
            rx_blk_send,
            rx_consensus_send,
            rx_pmaker_send,
            rx_blk_req_send,
            // rx_blk_resp_send,
        ));

        Ok((
            Self {
                transport_hub: send_half,
                neighbors,
                _peer_receiver_rt,
                _peer_receiver_handle,
            },
            rx_txn_recv,
            rx_blk_recv,
            rx_consensus_recv,
            rx_pmaker_recv,
            rx_blk_req_recv,
            // rx_blk_resp_recv,
        ))
    }

    pub async fn send(&self, dest: NodeId, msg: MsgType) -> Result<(), CopycatError> {
        if let Err(e) = self.transport_hub.send(dest, msg).await {
            return Err(CopycatError(format!("send to {dest} failed: {e:?}")));
        }
        Ok(())
    }

    pub async fn broadcast(&self, msg: MsgType) -> Result<(), CopycatError> {
        log::trace!("broadcasting {msg:?}");
        if let Err(e) = self.transport_hub.broadcast(msg).await {
            return Err(CopycatError(format!("broadcast failed: {e:?}")));
        }
        Ok(())
    }

    pub async fn gossip(
        &self,
        msg: MsgType,
        skipping: HashSet<NodeId>,
    ) -> Result<(), CopycatError> {
        log::trace!("gossiping {msg:?}");
        let dests: Vec<u64> = self.neighbors.difference(&skipping).cloned().collect();
        if dests.len() > 0 {
            if let Err(e) = self.transport_hub.multicast(dests, msg).await {
                return Err(CopycatError(format!("gossip failed: {e:?}")));
            }
        }
        Ok(())
    }

    async fn peer_receiver_thread(
        mut transport_hub: ClientStubRecvHalf<MsgType>,
        rx_txn_send: mpsc::Sender<(NodeId, Arc<Txn>)>,
        rx_blk_send: mpsc::Sender<(NodeId, Arc<Block>)>,
        rx_consensus_send: mpsc::Sender<(NodeId, Arc<Vec<u8>>)>,
        rx_pmaker_send: mpsc::Sender<(NodeId, Arc<Vec<u8>>)>,
        rx_blk_req_send: mpsc::Sender<(NodeId, Hash)>,
        // rx_blk_resp_send: mpsc::Sender<(NodeId, (Hash, Arc<Block>))>,
    ) {
        log::info!("peer receiver thread started");

        loop {
            match transport_hub.recv().await {
                Ok(msg) => {
                    let (src, content) = msg;
                    match content {
                        MsgType::NewTxn { txn } => {
                            if let Err(e) = rx_txn_send.send((src, Arc::new(txn))).await {
                                log::error!("rx_txn_send failed: {e:?}")
                            }
                        }
                        MsgType::NewBlock { blk } => {
                            if let Err(e) = rx_blk_send.send((src, Arc::new(blk))).await {
                                log::error!("rx_blk_send failed: {e:?}")
                            }
                        }
                        MsgType::ConsensusMsg { msg } => {
                            if let Err(e) = rx_consensus_send.send((src, Arc::new(msg))).await {
                                log::error!("rx_consensus_send failed: {e:?}")
                            }
                        }
                        MsgType::PMakerMsg { msg } => {
                            if let Err(e) = rx_pmaker_send.send((src, Arc::new(msg))).await {
                                log::error!("rx_pmaker_send failed: {e:?}")
                            }
                        }
                        MsgType::BlockReq { blk_id } => {
                            if let Err(e) = rx_blk_req_send.send((src, blk_id)).await {
                                log::error!("rx_pmaker_send failed: {e:?}")
                            }
                        } // MsgType::BlockResp { id, blk } => {
                          //     if let Err(e) = rx_blk_resp_send.send((src, (id, Arc::new(blk)))).await
                          //     {
                          //         log::error!("rx_pmaker_send failed: {e:?}")
                          //     }
                          // }
                    }
                }
                Err(e) => {
                    log::error!("got error listening to peers: {e:?}");
                }
            }
        }
    }
}
