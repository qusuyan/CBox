use crate::protocol::block::Block;
use crate::protocol::MsgType;
use crate::transaction::Txn;
use crate::{CopycatError, NodeId};

use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::Duration;

pub struct PeerMessenger {}

impl PeerMessenger {
    pub async fn new(
        _id: NodeId,
        _num_mailbox_workers: usize,
        _neighbors: HashSet<NodeId>,
    ) -> Result<
        (
            Self,
            mpsc::Receiver<(NodeId, Vec<Arc<Txn>>)>,
            mpsc::Receiver<(NodeId, Arc<Block>)>,
            mpsc::Receiver<(NodeId, Vec<u8>)>,
            mpsc::Receiver<(NodeId, Vec<u8>)>,
            mpsc::Receiver<(NodeId, Vec<u8>)>,
            // mpsc::Receiver<(NodeId, (Hash, Arc<Block>))>,
        ),
        CopycatError,
    > {
        unreachable!()
    }

    pub fn new_stub() -> Self {
        Self {}
    }

    pub async fn send(&self, dest: NodeId, msg: MsgType) -> Result<(), CopycatError> {
        println!("Sending msg to node {dest}: {msg:?}");
        Ok(())
    }

    pub async fn delayed_send(
        &self,
        dest: NodeId,
        msg: MsgType,
        delay: Duration,
    ) -> Result<(), CopycatError> {
        println!(
            "Sending msg to node {dest} with delay {}: {msg:?}",
            delay.as_secs_f64()
        );
        Ok(())
    }

    pub async fn broadcast(&self, msg: MsgType) -> Result<(), CopycatError> {
        println!("Broadcasting msg: {msg:?}");
        Ok(())
    }

    pub async fn gossip(
        &self,
        msg: MsgType,
        _skipping: HashSet<NodeId>,
    ) -> Result<(), CopycatError> {
        println!("Gossiping msg: {msg:?}");
        Ok(())
    }

    pub async fn sample(&self, msg: MsgType, neighbors: usize) -> Result<(), CopycatError> {
        println!("Sending msg to {neighbors} neighbors: {msg:?}");
        Ok(())
    }
}
