use copycat_protocol::{block::Block, transaction::Txn, MsgType};
use copycat_utils::{CopycatError, NodeId};
use mailbox_client::ClientStub;

use serde::{Deserialize, Serialize};
use tokio::{sync::mpsc, task::JoinHandle};

use std::{fmt::Debug, sync::Arc};

#[derive(Debug, Serialize, Deserialize)]
enum SendRequest {
    Send { dest: NodeId, msg: MsgType },
    Broadcast { msg: MsgType },
}

pub struct PeerMessenger {
    id: NodeId,
    tx_send: mpsc::UnboundedSender<SendRequest>,
    _messenger_handle: JoinHandle<()>,
}

impl PeerMessenger {
    pub async fn new(
        id: NodeId,
    ) -> Result<
        (
            Self,
            mpsc::UnboundedReceiver<(NodeId, Arc<Txn>)>,
            mpsc::UnboundedReceiver<(NodeId, Arc<Block>)>,
            mpsc::UnboundedReceiver<(NodeId, Arc<Vec<u8>>)>,
            mpsc::UnboundedReceiver<(NodeId, Arc<Vec<u8>>)>,
        ),
        CopycatError,
    > {
        let transport_hub = ClientStub::new(id)?;

        let (tx_send, tx_recv) = mpsc::unbounded_channel();
        let (rx_txn_send, rx_txn_recv) = mpsc::unbounded_channel();
        let (rx_blk_send, rx_blk_recv) = mpsc::unbounded_channel();
        let (rx_consensus_send, rx_consensus_recv) = mpsc::unbounded_channel();
        let (rx_pmaker_send, rx_pmaker_recv) = mpsc::unbounded_channel();

        let _messenger_handle = tokio::spawn(Self::peer_messenger_thread(
            id,
            transport_hub,
            tx_recv,
            rx_txn_send,
            rx_blk_send,
            rx_consensus_send,
            rx_pmaker_send,
        ));

        Ok((
            Self {
                id,
                tx_send,
                _messenger_handle,
            },
            rx_txn_recv,
            rx_blk_recv,
            rx_consensus_recv,
            rx_pmaker_recv,
        ))
    }

    pub async fn send(&self, dest: NodeId, msg: MsgType) -> Result<(), CopycatError> {
        if let Err(e) = self.tx_send.send(SendRequest::Send { dest, msg }) {
            Err(CopycatError(format!(
                "Node {}: send to {dest} failed: {e:?}",
                self.id
            )))
        } else {
            Ok(())
        }
    }

    pub async fn broadcast(&self, msg: MsgType) -> Result<(), CopycatError> {
        log::trace!("Node {}: broadcasting {msg:?}", self.id);
        if let Err(e) = self.tx_send.send(SendRequest::Broadcast { msg }) {
            Err(CopycatError(format!(
                "Node {}: broadcast failed: {e:?}",
                self.id
            )))
        } else {
            Ok(())
        }
    }

    async fn peer_messenger_thread(
        id: NodeId,
        transport_hub: ClientStub<MsgType>,
        mut tx_recv: mpsc::UnboundedReceiver<SendRequest>,
        rx_txn_send: mpsc::UnboundedSender<(NodeId, Arc<Txn>)>,
        rx_blk_send: mpsc::UnboundedSender<(NodeId, Arc<Block>)>,
        rx_consensus_send: mpsc::UnboundedSender<(NodeId, Arc<Vec<u8>>)>,
        rx_pmaker_send: mpsc::UnboundedSender<(NodeId, Arc<Vec<u8>>)>,
    ) {
        log::trace!("Node {id}: peer messenger thread started");

        loop {
            tokio::select! {
                local_req = tx_recv.recv() => {
                    match local_req {
                        Some(req) => {
                            log::trace!("Node {id}: got request {req:?}");
                            match req {
                                SendRequest::Send{dest, msg} => {
                                    if let Err(e) = transport_hub.send(dest, msg).await {
                                        log::error!("Node {id}: failed to send message to peer {dest}: {e:?}")
                                    }
                                },
                                SendRequest::Broadcast{msg} => {
                                    if let Err(e) = transport_hub.broadcast(msg).await {
                                        log::error!("Node {id}: failed to broadcast message to peers: {e:?}")
                                    }
                                },
                            }
                        }
                        None => {
                            log::error!("Node {id}: peer message channel closed");
                        }
                    }
                }
                peer_msg = transport_hub.recv() => {
                    match peer_msg {
                        Ok(msg) => {
                            let (src, content) = msg;
                            match content {
                                MsgType::NewTxn{txn} => {
                                    if let Err(e) = rx_txn_send.send((src, Arc::new(txn))) {
                                        log::error!("Node {id}: rx_txn_send failed: {e:?}")
                                    }
                                },
                                MsgType::NewBlock{blk} => {
                                    if let Err(e) = rx_blk_send.send((src, Arc::new(blk))) {
                                        log::error!("Node {id}: rx_blk_send failed: {e:?}")
                                    }
                                },
                                MsgType::ConsensusMsg{msg} => {
                                    if let Err(e) = rx_consensus_send.send((src, Arc::new(msg))) {
                                        log::error!("Node {id}: rx_consensus_send failed: {e:?}")
                                    }
                                },
                                MsgType::PMakerMsg{msg} => {
                                    if let Err(e) = rx_pmaker_send.send((src, Arc::new(msg))) {
                                        log::error!("Node {id}: rx_pmaker_send failed: {e:?}")
                                    }
                                },
                            }
                        },
                        Err(e) => {
                            log::error!("Node {id}: got error listening to peers: {e:?}");
                        }
                    }
                }
            }
        }
    }
}
