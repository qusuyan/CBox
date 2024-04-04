mod dummy;

mod bitcoin;
use bitcoin::BitcoinFlowGen;

mod avalanche;
use avalanche::AvalancheFlowGen;

use crate::protocol::{transaction::Txn, ChainType, CryptoScheme};
use crate::utils::{CopycatError, NodeId};
use async_trait::async_trait;

use std::sync::Arc;

pub struct Stats {
    pub latency: f64,
    pub num_committed: u64,
    pub chain_length: u64,
    pub commit_confidence: f64,
}

#[async_trait]
pub trait FlowGen {
    async fn setup_txns(&mut self) -> Result<Vec<Arc<Txn>>, CopycatError>;
    async fn wait_next(&self) -> Result<(), CopycatError>;
    async fn next_txn(&mut self) -> Result<Arc<Txn>, CopycatError>;
    async fn txn_committed(
        &mut self,
        node: NodeId,
        txns: Vec<Arc<Txn>>,
        blk_height: u64,
    ) -> Result<(), CopycatError>;
    fn get_stats(&self) -> Stats;
}

pub fn get_flow_gen(
    id: NodeId,
    num_accounts: usize,
    max_inflight: usize,
    frequency: usize,
    chain: ChainType,
    crypto: CryptoScheme,
) -> Box<dyn FlowGen> {
    match chain {
        ChainType::Bitcoin => Box::new(BitcoinFlowGen::new(
            id,
            num_accounts,
            max_inflight,
            frequency,
            crypto,
        )),
        ChainType::Avalanche => Box::new(AvalancheFlowGen::new(
            id,
            num_accounts,
            max_inflight,
            frequency,
            crypto,
        )),
        _ => todo!(),
    }
}
