use crate::protocol::ChainType;
use crate::utils::CopycatError;
use crate::{DissemPattern, NodeId};

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::BufReader;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ChainConfig {
    Dummy { config: DummyConfig },
    Bitcoin { config: BitcoinConfig },
    Avalanche { config: AvalancheConfig },
    ChainReplication { config: ChainReplicationConfig },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeConfig {
    pub chain_config: ChainConfig,
    pub max_concurrency: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DummyConfig {
    pub txn_dissem: DissemPattern,
    pub blk_dissem: DissemPattern,
}

impl Default for DummyConfig {
    fn default() -> Self {
        Self {
            txn_dissem: DissemPattern::Broadcast,
            blk_dissem: DissemPattern::Broadcast,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BitcoinConfig {
    pub difficulty: u8,
    pub compute_power: f64,
    pub commit_depth: u8,
    pub txn_dissem: DissemPattern,
    pub blk_dissem: DissemPattern,
}

impl Default for BitcoinConfig {
    fn default() -> Self {
        Self {
            difficulty: 25,
            compute_power: 1.0,
            commit_depth: 6,
            txn_dissem: DissemPattern::Gossip,
            blk_dissem: DissemPattern::Gossip,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AvalancheConfig {
    Correct { config: AvalancheCorrectConfig },
    VoteNo { config: AvalancheVoteNoConfig },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AvalancheCorrectConfig {
    pub blk_len: usize,
    pub k: usize,
    pub alpha: f64,
    pub beta1: u64,
    pub beta2: u64,
    pub proposal_timeout_secs: f64,
    pub max_inflight_blk: usize,
    pub txn_dissem: DissemPattern,
}

// https://arxiv.org/pdf/1906.08936.pdf
impl Default for AvalancheCorrectConfig {
    fn default() -> Self {
        Self {
            blk_len: 40,
            k: 10,
            alpha: 0.8,
            beta1: 11,
            beta2: 150,
            proposal_timeout_secs: 5.0,
            max_inflight_blk: 40, // 40 * blk_len ~ 1800 txns / blk (bitcoin)
            txn_dissem: DissemPattern::Broadcast,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AvalancheVoteNoConfig {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChainReplicationConfig {
    pub order: Vec<NodeId>,
    pub blk_size: usize,
}

impl Default for ChainReplicationConfig {
    fn default() -> Self {
        Self {
            order: vec![],
            blk_size: 0x100000,
        }
    }
}

impl NodeConfig {
    pub fn validate(&mut self, neighbors: &HashSet<NodeId>, all_nodes: &HashSet<NodeId>) {
        // TODO: validate dissem pattern as well
        match &mut self.chain_config {
            ChainConfig::Dummy { .. } => {}
            ChainConfig::Bitcoin { .. } => {}
            ChainConfig::Avalanche { config } => {
                match config {
                    AvalancheConfig::Correct { config } => {
                        let max_voters = neighbors.len() + 1; // including self
                        if config.k > max_voters {
                            log::warn!("not enough neighbors, setting k to {max_voters} instead");
                            config.k = max_voters;
                        }
                        if config.alpha <= 0.5 {
                            log::warn!(
                                "alpha has to be greater than 0.5 to ensure majority vote, setting to 0.51"
                            );
                            config.alpha = 0.51
                        }
                    }
                    AvalancheConfig::VoteNo { .. } => {} // do nothing
                }
            }
            ChainConfig::ChainReplication { config } => {
                let mut on_chain = HashSet::new();
                let mut valid = true;
                // make sure that nodes on chain are valid
                for node in config.order.iter() {
                    if all_nodes.contains(node) && !on_chain.contains(node) {
                        on_chain.insert(*node);
                    } else {
                        valid = false;
                        break;
                    }
                }
                if !valid {
                    let valid_order = all_nodes.iter().cloned().collect();
                    log::warn!("invalid order, setting to {valid_order:?} instead");
                    config.order = valid_order;
                }
                // make sure that all nodes are included in the chain
                for node in all_nodes.iter() {
                    if !on_chain.contains(node) {
                        config.order.push(*node)
                    }
                }
            }
        }
    }
}

impl ChainConfig {
    pub fn get_txn_dissem(&self) -> DissemPattern {
        match self {
            ChainConfig::Dummy { config } => config.txn_dissem.clone(),
            ChainConfig::Bitcoin { config } => config.txn_dissem.clone(),
            ChainConfig::Avalanche { config } => match config {
                AvalancheConfig::Correct { config } => config.txn_dissem.clone(),
                AvalancheConfig::VoteNo { .. } => DissemPattern::Passthrough,
            },
            ChainConfig::ChainReplication { .. } => DissemPattern::Passthrough,
        }
    }

    pub fn get_blk_dissem(&self) -> DissemPattern {
        match self {
            ChainConfig::Dummy { config } => config.blk_dissem.clone(),
            ChainConfig::Bitcoin { config } => config.blk_dissem.clone(),
            ChainConfig::Avalanche { config } => match config {
                AvalancheConfig::Correct { config } => DissemPattern::Sample {
                    sample_size: config.k,
                },
                AvalancheConfig::VoteNo { .. } => DissemPattern::Passthrough,
            },
            ChainConfig::ChainReplication { config } => DissemPattern::Linear {
                order: config.order.clone(),
            },
        }
    }
}

#[derive(Serialize, Deserialize)]
struct JsonConfigItem {
    nodes: Vec<NodeId>,
    config: serde_json::Value,
    max_concurrency: Option<usize>,
}

type JsonConfig = Vec<JsonConfigItem>;

pub fn parse_config_file(
    path: &String,
    chain_type: ChainType,
) -> Result<HashMap<NodeId, NodeConfig>, CopycatError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let config_json: JsonConfig = serde_json::from_reader(reader)?;

    let mut config_map = HashMap::new();
    for config_item in config_json {
        for node in config_item.nodes {
            let config = match chain_type {
                ChainType::Dummy => ChainConfig::Dummy {
                    config: serde_json::from_value(config_item.config.clone())?,
                },
                ChainType::Bitcoin => ChainConfig::Bitcoin {
                    config: serde_json::from_value(config_item.config.clone())?,
                },
                ChainType::Avalanche => ChainConfig::Avalanche {
                    config: serde_json::from_value(config_item.config.clone())?,
                },
                ChainType::ChainReplication => ChainConfig::ChainReplication {
                    config: serde_json::from_value(config_item.config.clone())?,
                },
            };
            config_map.insert(
                node,
                NodeConfig {
                    chain_config: config,
                    max_concurrency: config_item.max_concurrency,
                },
            );
        }
    }

    Ok(config_map)
}
