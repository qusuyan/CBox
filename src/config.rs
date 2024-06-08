use std::collections::{HashMap, HashSet};

use crate::protocol::ChainType;
use crate::utils::CopycatError;
use crate::{parsed_config, NodeId};

#[derive(Clone, Debug)]
pub enum Config {
    Dummy,
    Bitcoin { config: BitcoinConfig },
    Avalanche { config: AvalancheConfig },
    ChainReplication { config: ChainReplicationConfig },
}

#[derive(Clone, Debug)]
pub struct BitcoinConfig {
    pub difficulty: u8,
    pub compute_power: f64,
    pub commit_depth: u8,
}

impl Default for BitcoinConfig {
    fn default() -> Self {
        Self {
            difficulty: 25,
            compute_power: 1.0,
            commit_depth: 6,
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
            ChainType::Dummy => Ok(Config::Dummy),
            ChainType::Bitcoin => Ok(Config::Bitcoin {
                config: parsed_config!(input => BitcoinConfig; difficulty, commit_depth, compute_power)?,
            }),
            ChainType::Avalanche => Ok(Config::Avalanche {
                config: parsed_config!(input => AvalancheConfig; blk_len, k, alpha, beta1, beta2, proposal_timeout_secs, max_inflight_blk)?,
            }),
            ChainType::ChainReplication => Ok(Config::ChainReplication {
                config: parsed_config!(input => ChainReplicationConfig; order)?,
            }),
        }
    }

    pub fn validate(&mut self, topology: &HashMap<NodeId, HashSet<NodeId>>) {
        match self {
            Config::Dummy => {}
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
                for node in config.order.iter() {
                    if topology.contains_key(node) && !on_chain.contains(node) {
                        on_chain.insert(node);
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
            }
        }
    }
}
