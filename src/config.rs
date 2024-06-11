use std::collections::{HashMap, HashSet};

use clap::ValueEnum;

use crate::protocol::ChainType;
use crate::utils::CopycatError;
use crate::{parsed_config, DissemPattern, NodeId};

#[derive(Clone, Debug)]
pub enum Config {
    Dummy { config: DummyConfig },
    Bitcoin { config: BitcoinConfig },
    Avalanche { config: AvalancheConfig },
    ChainReplication { config: ChainReplicationConfig },
}

#[derive(Clone, Debug)]
pub struct DummyConfig {
    pub txn_dissem: String,
    pub blk_dissem: String,
}

impl Default for DummyConfig {
    fn default() -> Self {
        Self {
            txn_dissem: String::from("broadcast"),
            blk_dissem: String::from("broadcast"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct BitcoinConfig {
    pub difficulty: u8,
    pub compute_power: f64,
    pub commit_depth: u8,
    pub txn_dissem: String,
    pub blk_dissem: String,
}

impl Default for BitcoinConfig {
    fn default() -> Self {
        Self {
            difficulty: 25,
            compute_power: 1.0,
            commit_depth: 6,
            txn_dissem: String::from("gossip"),
            blk_dissem: String::from("gossip"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct AvalancheConfig {
    pub blk_len: usize,
    pub k: usize,
    pub alpha: f64,
    pub beta1: u64,
    pub beta2: u64,
    pub proposal_timeout_secs: f64,
    pub max_inflight_blk: usize,
    pub txn_dissem: String,
}

// https://arxiv.org/pdf/1906.08936.pdf
impl Default for AvalancheConfig {
    fn default() -> Self {
        Self {
            blk_len: 40,
            k: 10,
            alpha: 0.8,
            beta1: 11,
            beta2: 150,
            proposal_timeout_secs: 5.0,
            max_inflight_blk: 40, // 40 * blk_len ~ 1800 txns / blk (bitcoin)
            txn_dissem: String::from("broadcast"),
        }
    }
}

#[derive(Clone, Debug)]
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

impl Config {
    pub fn from_str(chain_type: ChainType, input: Option<&str>) -> Result<Self, CopycatError> {
        match chain_type {
            ChainType::Dummy => Ok(Config::Dummy {
                config: parsed_config!(input => DummyConfig; txn_dissem, blk_dissem)?,
            }),
            ChainType::Bitcoin => Ok(Config::Bitcoin {
                config: parsed_config!(input => BitcoinConfig; difficulty, commit_depth, compute_power, txn_dissem, blk_dissem)?,
            }),
            ChainType::Avalanche => Ok(Config::Avalanche {
                config: parsed_config!(input => AvalancheConfig; blk_len, k, alpha, beta1, beta2, proposal_timeout_secs, max_inflight_blk, txn_dissem)?,
            }),
            ChainType::ChainReplication => Ok(Config::ChainReplication {
                config: parsed_config!(input => ChainReplicationConfig; order)?,
            }),
        }
    }

    pub fn validate(&mut self, topology: &HashMap<NodeId, HashSet<NodeId>>) {
        match self {
            Config::Dummy { .. } => {}
            Config::Bitcoin { .. } => {}
            Config::Avalanche { config } => {
                let min_neighbors = topology
                    .iter()
                    .map(|(_, neighbors)| neighbors.len())
                    .min()
                    .unwrap();
                let max_voters = min_neighbors + 1;
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
            Config::ChainReplication { config } => {
                let mut on_chain = HashSet::new();
                let mut valid = true;
                // make sure that nodes on chain are valid
                for node in config.order.iter() {
                    if topology.contains_key(node) && !on_chain.contains(node) {
                        on_chain.insert(*node);
                    } else {
                        valid = false;
                        break;
                    }
                }
                if !valid {
                    let valid_order = topology.keys().cloned().collect();
                    log::warn!("invalid order, setting to {valid_order:?} instead");
                    config.order = valid_order;
                }
                // make sure that all nodes are included in the chain
                for node in topology.keys() {
                    if !on_chain.contains(node) {
                        config.order.push(*node)
                    }
                }
            }
        }
        self.validate_dissem_patterns();
    }

    fn validate_dissem_patterns(&mut self) {
        if let Err(e) = self.get_txn_dissem_inner() {
            log::warn!("Invalid txn dissem pattern: {e}, restore to default");
            match self {
                Config::Dummy { config } => config.txn_dissem = String::from("broadcast"),
                Config::Bitcoin { config } => config.txn_dissem = String::from("gossip"),
                Config::Avalanche { config } => config.txn_dissem = String::from("broadcast"),
                Config::ChainReplication { .. } => {}
            }
        }

        if let Err(e) = self.get_blk_dissem_inner() {
            log::warn!("Invalid blk dissem pattern: {e}, restore to default");
            match self {
                Config::Dummy { config } => config.blk_dissem = String::from("broadcast"),
                Config::Bitcoin { config } => config.blk_dissem = String::from("gossip"),
                Config::ChainReplication { .. } | Config::Avalanche { .. } => {}
            }
        }
    }

    fn get_txn_dissem_inner(&self) -> Result<DissemPattern, String> {
        match self {
            Config::Dummy { config } => DissemPattern::from_str(&config.txn_dissem, true),
            Config::Bitcoin { config } => DissemPattern::from_str(&config.txn_dissem, true),
            Config::Avalanche { config } => DissemPattern::from_str(&config.txn_dissem, true),
            Config::ChainReplication { .. } => Ok(DissemPattern::Passthrough),
        }
    }

    pub fn get_txn_dissem(&self) -> DissemPattern {
        self.get_txn_dissem_inner().unwrap()
    }

    fn get_blk_dissem_inner(&self) -> Result<DissemPattern, String> {
        match self {
            Config::Dummy { config } => DissemPattern::from_str(&config.blk_dissem, true),
            Config::Bitcoin { config } => DissemPattern::from_str(&config.blk_dissem, true),
            Config::Avalanche { .. } => Ok(DissemPattern::Sample),
            Config::ChainReplication { .. } => Ok(DissemPattern::Linear),
        }
    }

    pub fn get_blk_dissem(&self) -> DissemPattern {
        self.get_blk_dissem_inner().unwrap()
    }
}
