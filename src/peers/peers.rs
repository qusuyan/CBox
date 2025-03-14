use crate::get_report_timer;
use crate::protocol::{block::Block, transaction::Txn, MsgType};
use crate::utils::{CopycatError, NodeId};
use mailbox_client::{ClientStubRecvHalf, ClientStubSendHalf};

use rand::seq::{IteratorRandom, SliceRandom};

use tokio::time::Duration;
use tokio::{sync::mpsc, task::JoinHandle};

use std::collections::HashSet;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::sync::Arc;

pub struct PeerMessenger {
    id: NodeId,
    transport_hub: ClientStubSendHalf<MsgType>,
    neighbors: HashSet<NodeId>,
    msgs_sent: Arc<AtomicU64>,
    _peer_receiver_rt: Option<tokio::runtime::Runtime>,
    _peer_receiver_handle: JoinHandle<()>,
}

impl PeerMessenger {
    pub async fn new(
        id: NodeId,
        num_mailbox_workers: usize,
        neighbors: HashSet<NodeId>,
    ) -> Result<
        (
            Self,
            mpsc::Receiver<(NodeId, Vec<Arc<Txn>>)>,
            mpsc::Receiver<(NodeId, Arc<Block>)>,
            mpsc::Receiver<(NodeId, Vec<u8>)>,
            mpsc::Receiver<(NodeId, Vec<u8>)>,
            mpsc::Receiver<(NodeId, Vec<u8>)>,
            mpsc::Receiver<(NodeId, Vec<u8>)>,
            // mpsc::Receiver<(NodeId, (Hash, Arc<Block>))>,
        ),
        CopycatError,
    > {
        let (send_half, recv_half) = mailbox_client::new_stub(id, num_mailbox_workers).await?;

        let (rx_txn_send, rx_txn_recv) = mpsc::channel(0x1000000);
        let (rx_blk_send, rx_blk_recv) = mpsc::channel(0x100000);
        let (rx_blk_dissem_send, rx_blk_dissem_recv) = mpsc::channel(0x100000);
        let (rx_consensus_send, rx_consensus_recv) = mpsc::channel(0x100000);
        let (rx_pmaker_send, rx_pmaker_recv) = mpsc::channel(0x100000);
        let (rx_blk_req_send, rx_blk_req_recv) = mpsc::channel(0x100000);
        // let (rx_blk_resp_send, rx_blk_resp_recv) = mpsc::channel(0x100000);

        let msgs_sent = Arc::new(AtomicU64::new(0));

        // put receiver runtime on a separate thread for the bug here: https://github.com/tokio-rs/tokio/issues/4730
        let (_peer_receiver_handle, _peer_receiver_rt) = if cfg!(feature = "interprocess") {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .thread_name(format!("copycat-server-{}-recver", id))
                .build()
                .unwrap();
            let _peer_receiver_handle = runtime.spawn(Self::peer_receiver_thread(
                id,
                recv_half,
                rx_txn_send,
                rx_blk_send,
                rx_blk_dissem_send,
                rx_consensus_send,
                rx_pmaker_send,
                rx_blk_req_send,
                msgs_sent.clone(),
                // rx_blk_resp_send,
            ));
            (_peer_receiver_handle, Some(runtime))
        } else {
            let _peer_receiver_handle = tokio::spawn(Self::peer_receiver_thread(
                id,
                recv_half,
                rx_txn_send,
                rx_blk_send,
                rx_blk_dissem_send,
                rx_consensus_send,
                rx_pmaker_send,
                rx_blk_req_send,
                msgs_sent.clone(),
                // rx_blk_resp_send,
            ));
            (_peer_receiver_handle, None)
        };

        Ok((
            Self {
                id,
                transport_hub: send_half,
                neighbors,
                msgs_sent,
                _peer_receiver_rt,
                _peer_receiver_handle,
            },
            rx_txn_recv,
            rx_blk_recv,
            rx_blk_dissem_recv,
            rx_consensus_recv,
            rx_pmaker_recv,
            rx_blk_req_recv,
            // rx_blk_resp_recv,
        ))
    }

    pub async fn send(&self, dest: NodeId, msg: MsgType) -> Result<(), CopycatError> {
        pf_trace!(self.id; "sending {:?} to {}", msg, dest);
        if let Err(e) = self.transport_hub.send(dest, msg).await {
            return Err(CopycatError(format!("send to {dest} failed: {e:?}")));
        }
        self.msgs_sent.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    pub async fn delayed_send(
        &self,
        dest: NodeId,
        msg: MsgType,
        delay: Duration,
    ) -> Result<(), CopycatError> {
        pf_trace!(self.id; "sending {:?} to {}", msg, dest);
        if let Err(e) = self.transport_hub.delayed_send(dest, msg, delay).await {
            return Err(CopycatError(format!("send to {dest} failed: {e:?}")));
        }
        self.msgs_sent.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    pub async fn broadcast(&self, msg: MsgType) -> Result<(), CopycatError> {
        pf_trace!(self.id; "broadcasting {:?}", msg);
        if let Err(e) = self.transport_hub.broadcast(msg).await {
            return Err(CopycatError(format!("broadcast failed: {e:?}")));
        }
        self.msgs_sent.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    pub async fn gossip(
        &self,
        msg: MsgType,
        skipping: HashSet<NodeId>,
    ) -> Result<(), CopycatError> {
        pf_trace!(self.id; "gossiping {:?}", msg);
        let dests: Vec<u64> = self.neighbors.difference(&skipping).cloned().collect();
        if dests.len() > 0 {
            if let Err(e) = self.transport_hub.multicast(dests, msg).await {
                return Err(CopycatError(format!("gossip failed: {e:?}")));
            }
            self.msgs_sent.fetch_add(1, Ordering::Relaxed);
        }
        Ok(())
    }

    pub async fn sample(&self, msg: MsgType, neighbors: usize) -> Result<(), CopycatError> {
        let sample = {
            let mut rng = rand::thread_rng();
            let mut samples = self.neighbors.iter().choose_multiple(&mut rng, neighbors);
            samples.shuffle(&mut rng);
            samples
        };
        let dests: Vec<u64> = sample.into_iter().cloned().collect();
        pf_trace!(self.id; "sending {:?} to {} neighbors ({:?})", msg, neighbors, dests);
        if dests.len() > 0 {
            if let Err(e) = self.transport_hub.multicast(dests, msg).await {
                return Err(CopycatError(format!("sample multicast failed: {e:?}")));
            }
            self.msgs_sent.fetch_add(1, Ordering::Relaxed);
        }
        Ok(())
    }

    async fn peer_receiver_thread(
        id: NodeId,
        mut transport_hub: ClientStubRecvHalf<MsgType>,
        rx_txn_send: mpsc::Sender<(NodeId, Vec<Arc<Txn>>)>,
        rx_blk_send: mpsc::Sender<(NodeId, Arc<Block>)>,
        rx_blk_dissem_send: mpsc::Sender<(NodeId, Vec<u8>)>,
        rx_consensus_send: mpsc::Sender<(NodeId, Vec<u8>)>,
        rx_pmaker_send: mpsc::Sender<(NodeId, Vec<u8>)>,
        rx_blk_req_send: mpsc::Sender<(NodeId, Vec<u8>)>,
        msgs_sent: Arc<AtomicU64>,
        // rx_blk_resp_send: mpsc::Sender<(NodeId, (Hash, Arc<Block>))>,
    ) {
        pf_info!(id; "peer receiver thread started");

        let mut report_timer = get_report_timer();
        let mut msgs_recv = 0;

        loop {
            tokio::select! {
                // TODO: hold semaphore to account for deserialization cost
                msg_recv = transport_hub.recv() => {
                    match msg_recv {
                        Ok(msg) => {
                            msgs_recv += 1;
                            let (src, content) = msg;
                            match content {
                                MsgType::NewTxn { txn_batch } => {
                                    if let Err(e) = rx_txn_send.send((src, txn_batch)).await {
                                        pf_error!(id; "rx_txn_send failed: {:?}", e)
                                    }
                                }
                                MsgType::NewBlock { blk } => {
                                    if let Err(e) = rx_blk_send.send((src, blk)).await {
                                        pf_error!(id; "rx_blk_send failed: {:?}", e)
                                    }
                                }
                                MsgType::BlkDissemMsg { msg } => {
                                    if let Err(e) = rx_blk_dissem_send.send((src, msg)).await {
                                        pf_error!(id; "rx_blk_send failed: {:?}", e)
                                    }
                                }
                                MsgType::ConsensusMsg { msg } => {
                                    if let Err(e) = rx_consensus_send.send((src, msg)).await {
                                        pf_error!(id; "rx_consensus_send failed: {:?}", e)
                                    }
                                }
                                MsgType::PMakerMsg { msg } => {
                                    if let Err(e) = rx_pmaker_send.send((src, msg)).await {
                                        pf_error!(id; "rx_pmaker_send failed: {:?}", e)
                                    }
                                }
                                MsgType::BlockReq { msg } => {
                                    if let Err(e) = rx_blk_req_send.send((src, msg)).await {
                                        pf_error!(id; "rx_pmaker_send failed: {:?}", e)
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            pf_error!(id; "got error listening to peers: {:?}", e);
                        }
                    }
                },

                report_val = report_timer.changed() => {
                    if let Err(e) = report_val {
                        pf_error!(id; "Waiting for report timeout failed: {}", e);
                    }

                    let num_msgs_sent = msgs_sent.swap(0, Ordering::Relaxed);
                    pf_info!(id; "In the last minute: {} msgs sent and {} msgs recved", num_msgs_sent, msgs_recv);
                    msgs_recv = 0;
                }
            }
        }
    }
}
