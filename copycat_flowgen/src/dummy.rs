use super::{FlowGen, Stats};
use crate::{ClientId, FlowGenId};
use copycat::protocol::crypto::Hash;
use copycat::protocol::transaction::{DummyTxn, Txn};
use copycat::{CopycatError, NodeId};

use async_trait::async_trait;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::{Duration, Instant};

pub struct DummyFlowGen {
    max_inflight: usize,
    batch_frequency: usize,
    batch_size: usize,
    next_batch_time: Instant,
    client_list: Vec<ClientId>,
    in_flight: HashMap<Hash, Instant>,
}

impl DummyFlowGen {
    pub fn new() -> Self {
        todo!()
    }
}

#[async_trait]
impl FlowGen for DummyFlowGen {
    async fn setup_txns(&mut self) -> Result<Vec<(ClientId, Arc<Txn>)>, CopycatError> {
        todo!()
    }

    async fn wait_next(&self) -> Result<(), CopycatError> {
        todo!()
    }

    async fn next_txn_batch(&mut self) -> Result<Vec<(ClientId, Arc<Txn>)>, CopycatError> {
        todo!()
    }

    async fn txn_committed(
        &mut self,
        node: NodeId,
        txns: Vec<Arc<Txn>>,
        blk_height: u64,
    ) -> Result<(), CopycatError> {
        todo!()
    }

    fn get_stats(&self) -> Stats {
        todo!()
    }
}
