use super::Decision;
use crate::config::BitcoinPBSConfig;
use crate::context::BlkCtx;
use crate::protocol::block::Block;
use crate::protocol::transaction::Txn;
use crate::{CopycatError, NodeId};

use std::sync::Arc;

use async_trait::async_trait;

pub struct BitcoinBuilderDecision {
    id: NodeId,
}

impl BitcoinBuilderDecision {
    pub fn new(id: NodeId, config: BitcoinPBSConfig) -> Self {
        Self { id }
    }
}

#[async_trait]
impl Decision for BitcoinBuilderDecision {
    async fn new_tail(
        &mut self,
        src: NodeId,
        new_tail: Vec<(Arc<Block>, Arc<BlkCtx>)>,
    ) -> Result<(), CopycatError> {
        todo!();
    }

    async fn commit_ready(&self) -> Result<(), CopycatError> {
        todo!();
    }

    async fn next_to_commit(&mut self) -> Result<(u64, Vec<Arc<Txn>>), CopycatError> {
        todo!();
    }

    async fn handle_peer_msg(&mut self, src: NodeId, content: Vec<u8>) -> Result<(), CopycatError> {
        todo!()
    }

    fn report(&mut self) {}
}
