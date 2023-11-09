use copycat_protocol::MsgType;
use copycat_utils::{CopycatError, NodeId};
use mailbox_client::ClientStub;

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::{sync::mpsc, task::JoinHandle};

use std::{fmt::Debug, sync::Arc};

#[derive(Debug, Serialize, Deserialize)]
enum SendRequest<TxnType, BlockType> {
    Send {
        dest: NodeId,
        msg: MsgType<TxnType, BlockType>,
    },
    Broadcast {
        msg: MsgType<TxnType, BlockType>,
    },
}

pub struct PeerMessenger<TxnType, BlockType> {
    id: NodeId,
    tx_send: mpsc::UnboundedSender<SendRequest<TxnType, BlockType>>,
    _messenger_handle: JoinHandle<()>,
}

impl<TxnType, BlockType> PeerMessenger<TxnType, BlockType>
where
    TxnType: 'static + Debug + Serialize + DeserializeOwned + Sync + Send,
    BlockType: 'static + Debug + Serialize + DeserializeOwned + Sync + Send,
{
    pub async fn new(
        id: NodeId,
    ) -> Result<
        (
            Self,
            mpsc::UnboundedReceiver<(NodeId, Arc<TxnType>)>,
            mpsc::UnboundedReceiver<(NodeId, Arc<BlockType>)>,
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

    pub async fn send(
        &self,
        dest: NodeId,
        msg: MsgType<TxnType, BlockType>,
    ) -> Result<(), CopycatError> {
        if let Err(e) = self.tx_send.send(SendRequest::Send { dest, msg }) {
            Err(CopycatError(format!(
                "Node {}: send to {dest} failed: {e:?}",
                self.id
            )))
        } else {
            Ok(())
        }
    }

    pub async fn broadcast(&self, msg: MsgType<TxnType, BlockType>) -> Result<(), CopycatError> {
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
        transport_hub: ClientStub<MsgType<TxnType, BlockType>>,
        mut tx_recv: mpsc::UnboundedReceiver<SendRequest<TxnType, BlockType>>,
        rx_txn_send: mpsc::UnboundedSender<(NodeId, Arc<TxnType>)>,
        rx_blk_send: mpsc::UnboundedSender<(NodeId, Arc<BlockType>)>,
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
