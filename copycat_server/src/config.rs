use copycat_protocol::ChainType;
use copycat_utils::{parsed_config, CopycatError};

#[derive(Clone, Debug)]
pub enum Config {
    Dummy,
    Bitcoin { config: BitcoinConfig },
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

impl Config {
    pub fn from_str(chain_type: ChainType, input: Option<&str>) -> Result<Self, CopycatError> {
        match chain_type {
            ChainType::Dummy => Ok(Config::Dummy),
            ChainType::Bitcoin => Ok(Config::Bitcoin {
                config: parsed_config!(input => BitcoinConfig; difficulty, commit_depth, compute_power)?,
            }),
        }
    }
}
