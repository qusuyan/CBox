use crate::parsed_config;
use crate::protocol::ChainType;
use crate::utils::CopycatError;

#[derive(Clone, Debug)]
pub enum Config {
    Dummy,
    Bitcoin { config: BitcoinConfig },
    Avalanche { config: AvalancheConfig },
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
    pub commit_timeout_secs: f64,
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
            commit_timeout_secs: 5.0,
            max_inflight_blk: 40, // 40 * blk_len ~ 1800 txns / blk (bitcoin)
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
                config: parsed_config!(input => AvalancheConfig; blk_len, k, alpha, beta1, beta2, proposal_timeout_secs, commit_timeout_secs, max_inflight_blk)?,
            }),
        }
    }

    pub fn validate(&mut self, num_neighbors: usize) {
        match self {
            Config::Dummy => {}
            Config::Bitcoin { .. } => {}
            Config::Avalanche { config } => {
                let max_voters = num_neighbors + 1;
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
        }
    }
}
