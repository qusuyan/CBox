mod dummy;

mod bitcoin;
pub use bitcoin::BitcoinFlowGen;

use async_trait::async_trait;
use copycat_protocol::{transaction::Txn, ChainType, CryptoScheme};
use copycat_utils::{CopycatError, NodeId};

use std::sync::Arc;

pub struct Stats {
    pub latency: f64,
    pub num_committed: u64,
}

#[async_trait]
pub trait FlowGen {
    async fn setup_txns(&mut self) -> Result<Vec<Arc<Txn>>, CopycatError>;
    async fn wait_next(&self) -> Result<(), CopycatError>;
    async fn next_txn(&mut self) -> Result<Arc<Txn>, CopycatError>;
    async fn txn_committed(&mut self, txn: Arc<Txn>) -> Result<(), CopycatError>;
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
        _ => todo!(),
    }
}
