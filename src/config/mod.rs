mod dummy;
pub use dummy::DummyConfig;

mod avalanche;
pub use avalanche::{AvalancheBasicConfig, AvalancheConfig, AvalancheVoteNoConfig};

mod bitcoin;
pub use bitcoin::{BitcoinBasicConfig, BitcoinConfig};

mod chain_replication;
pub use chain_replication::ChainReplicationConfig;

mod diem;
pub use diem::{DiemBasicConfig, DiemConfig};

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
    Diem { config: DiemConfig },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeConfig {
    pub chain_config: ChainConfig,
    pub max_concurrency: Option<usize>,
}

impl NodeConfig {
    pub fn validate(&mut self, neighbors: &HashSet<NodeId>, all_nodes: &HashSet<NodeId>) {
        // TODO: validate dissem pattern as well
        match &mut self.chain_config {
            ChainConfig::Dummy { .. } => {}
            ChainConfig::Bitcoin { .. } => {}
            ChainConfig::Avalanche { config } => {
                match config {
                    AvalancheConfig::Basic { config } => {
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
                    AvalancheConfig::Blizzard { config } => {
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
                    let mut valid_order: Vec<NodeId> = all_nodes.iter().cloned().collect();
                    valid_order.sort();
                    log::warn!("invalid order, setting to {valid_order:?} instead");
                    config.order = valid_order;
                } else {
                    // make sure that all nodes are included in the chain
                    for node in all_nodes.iter() {
                        if !on_chain.contains(node) {
                            config.order.push(*node)
                        }
                    }
                }
            }
            ChainConfig::Diem { .. } => {}
        }
    }
}

impl ChainConfig {
    pub fn get_txn_dissem(&self) -> DissemPattern {
        match self {
            ChainConfig::Dummy { config } => config.txn_dissem.clone(),
            ChainConfig::Bitcoin { config } => match config {
                BitcoinConfig::Basic { config } => config.txn_dissem.clone(),
                BitcoinConfig::Eager { config } => config.txn_dissem.clone(),
            },
            ChainConfig::Avalanche { config } => match config {
                AvalancheConfig::Basic { config } => config.txn_dissem.clone(),
                AvalancheConfig::Blizzard { config } => config.txn_dissem.clone(),
                AvalancheConfig::VoteNo { .. } => DissemPattern::Passthrough,
            },
            ChainConfig::ChainReplication { .. } => DissemPattern::Passthrough,
            ChainConfig::Diem { config } => match config {
                DiemConfig::Basic { config } => config.txn_dissem.clone(),
            },
        }
    }

    pub fn get_blk_dissem(&self) -> DissemPattern {
        match self {
            ChainConfig::Dummy { config } => config.blk_dissem.clone(),
            ChainConfig::Bitcoin { config } => match config {
                BitcoinConfig::Basic { config } => config.blk_dissem.clone(),
                BitcoinConfig::Eager { config } => config.blk_dissem.clone(),
            },
            ChainConfig::Avalanche { config } => match config {
                AvalancheConfig::Basic { config } => DissemPattern::Sample {
                    sample_size: config.k,
                },
                AvalancheConfig::Blizzard { config } => DissemPattern::Sample {
                    sample_size: config.k,
                },
                AvalancheConfig::VoteNo { .. } => DissemPattern::Passthrough,
            },
            ChainConfig::ChainReplication { config } => DissemPattern::Linear {
                order: config.order.clone(),
            },
            ChainConfig::Diem { .. } => DissemPattern::Broadcast,
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
        println!(":?",);
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
            ChainType::Diem => ChainConfig::Diem {
                config: serde_json::from_value(config_item.config.clone())?,
            },
        };
        println!("{:?}", config);
        for node in config_item.nodes {
            config_map.insert(
                node,
                NodeConfig {
                    chain_config: config.clone(),
                    max_concurrency: config_item.max_concurrency,
                },
            );
        }
    }

    Ok(config_map)
}
