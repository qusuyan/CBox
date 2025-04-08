mod chain_replication;
use chain_replication::ChainReplicationFlowGen;

mod bitcoin;
use bitcoin::BitcoinFlowGen;

mod avalanche;
use avalanche::AvalancheFlowGen;

mod diem;
use diem::DiemFlowGen;

mod aptos;
use aptos::AptosFlowGen;

use async_trait::async_trait;
use lazy_static::lazy_static;
use serde::Serialize;

use copycat::{
    protocol::crypto::{PrivKey, Signature},
    transaction::Txn,
    ChainType, CopycatError, NodeId, SignatureScheme,
};

use std::sync::Arc;

pub type FlowGenId = u64;
pub type ClientId = u64;

lazy_static! {
    static ref LAT_SAMPLE_RATE: f64 = std::env::var("LAT_SAMPLE_RATE")
        .map(|rate| match rate.parse::<f64>() {
            Ok(r) => Some(r),
            Err(e) => {
                log::warn!("Invalid LAT_SAMPLE_RATE: {e:?}, using default");
                None
            }
        })
        .unwrap_or(None)
        .unwrap_or(0.1);
}

pub struct Stats {
    pub num_committed: u64,
    pub chain_length: u64,
    pub commit_confidence: f64,
    pub inflight_txns: usize,
    pub latencies: Vec<f64>,
}

#[async_trait]
pub trait FlowGen {
    async fn setup_txns(&mut self) -> Result<Vec<(ClientId, Arc<Txn>)>, CopycatError>;
    async fn wait_next(&self) -> Result<(), CopycatError>;
    async fn next_txn_batch(&mut self) -> Result<Vec<(ClientId, Arc<Txn>)>, CopycatError>;
    async fn txn_committed(
        &mut self,
        node: NodeId,
        txns: Vec<Arc<Txn>>,
        blk_height: u64,
    ) -> Result<(), CopycatError>;
    fn get_stats(&mut self) -> Stats;
}

pub fn get_flow_gen(
    id: FlowGenId,
    client_list: Vec<ClientId>,
    num_accounts: usize,
    script_size: Option<usize>,
    max_inflight: usize,
    frequency: usize,
    conflict_rate: f64,
    chain: ChainType,
    crypto: SignatureScheme,
) -> Box<dyn FlowGen> {
    match chain {
        ChainType::Dummy => todo!(),
        ChainType::Bitcoin => Box::new(BitcoinFlowGen::new(
            id,
            client_list,
            num_accounts,
            script_size,
            max_inflight,
            frequency,
            crypto,
        )),
        ChainType::Avalanche => Box::new(AvalancheFlowGen::new(
            id,
            client_list,
            num_accounts,
            script_size,
            max_inflight,
            frequency,
            conflict_rate,
            crypto,
        )),
        ChainType::ChainReplication => Box::new(ChainReplicationFlowGen::new(
            client_list,
            script_size,
            max_inflight,
            frequency,
            crypto,
        )),
        ChainType::Diem => Box::new(DiemFlowGen::new(
            id,
            client_list,
            num_accounts,
            script_size,
            max_inflight,
            frequency,
            crypto,
        )),
        ChainType::Aptos => Box::new(AptosFlowGen::new(
            id,
            client_list,
            num_accounts,
            script_size,
            max_inflight,
            frequency,
            crypto,
        )),
    }
}

fn mock_sign<T: Serialize>(
    scheme: SignatureScheme,
    sk: &PrivKey,
    input: &T,
) -> Result<Signature, CopycatError> {
    let serialized = match scheme {
        SignatureScheme::Dummy | SignatureScheme::DummyECDSA => {
            // avoid extra serialization
            vec![]
        }
        SignatureScheme::ECDSA => bincode::serialize(&input)?,
    };

    let (signature, _) = scheme.sign(sk, &serialized)?;
    Ok(signature)
}
