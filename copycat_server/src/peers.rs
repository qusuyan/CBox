use copycat_protocol::{block::Block, transaction::Txn, MsgType};
use copycat_utils::{CopycatError, NodeId};
use mailbox_client::{ClientStubRecvHalf, ClientStubSendHalf};

use serde::{Deserialize, Serialize};
use tokio::{sync::mpsc, task::JoinHandle};

use std::{fmt::Debug, sync::Arc};

#[derive(Debug, Serialize, Deserialize)]
enum SendRequest {
    Send { dest: NodeId, msg: MsgType },
    Broadcast { msg: MsgType },
}

pub struct PeerMessenger {
    transport_hub: ClientStubSendHalf<MsgType>,
    _peer_receiver_rt: tokio::runtime::Runtime,
    _peer_receiver_handle: JoinHandle<()>,
}

impl PeerMessenger {
    pub async fn new(
        id: NodeId,
    ) -> Result<
        (
            Self,
            mpsc::Receiver<(NodeId, Arc<Txn>)>,
            mpsc::Receiver<(NodeId, Arc<Block>)>,
            mpsc::Receiver<(NodeId, Arc<Vec<u8>>)>,
            mpsc::Receiver<(NodeId, Arc<Vec<u8>>)>,
        ),
        CopycatError,
    > {
        let (send_half, recv_half) = mailbox_client::new_stub(id)?;

        let (rx_txn_send, rx_txn_recv) = mpsc::channel(0x1000000);
        let (rx_blk_send, rx_blk_recv) = mpsc::channel(0x100000);
        let (rx_consensus_send, rx_consensus_recv) = mpsc::channel(0x100000);
        let (rx_pmaker_send, rx_pmaker_recv) = mpsc::channel(0x100000);

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
        ));

        Ok((
            Self {
                transport_hub: send_half,
                _peer_receiver_rt,
                _peer_receiver_handle,
            },
            rx_txn_recv,
            rx_blk_recv,
            rx_consensus_recv,
            rx_pmaker_recv,
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

    async fn peer_receiver_thread(
        mut transport_hub: ClientStubRecvHalf<MsgType>,
        rx_txn_send: mpsc::Sender<(NodeId, Arc<Txn>)>,
        rx_blk_send: mpsc::Sender<(NodeId, Arc<Block>)>,
        rx_consensus_send: mpsc::Sender<(NodeId, Arc<Vec<u8>>)>,
        rx_pmaker_send: mpsc::Sender<(NodeId, Arc<Vec<u8>>)>,
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
                    }
                }
                Err(e) => {
                    log::error!("got error listening to peers: {e:?}");
                }
            }
        }
    }
}
