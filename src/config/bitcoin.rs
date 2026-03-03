use crate::DissemPattern;

use default_fields::DefaultFields;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BitcoinConfig {
    Basic { config: BitcoinBasicConfig },
    Eager { config: BitcoinBasicConfig },
}

#[derive(Clone, Debug, Serialize, Deserialize, DefaultFields)]
pub struct BitcoinBasicConfig {
    #[serde(default = "BitcoinBasicConfig::get_default_blk_size")]
    pub blk_size: usize,
    #[serde(default = "BitcoinBasicConfig::get_default_difficulty")]
    pub difficulty: u8,
    #[serde(default = "BitcoinBasicConfig::get_default_commit_depth")]
    pub commit_depth: u8,
    #[serde(default = "BitcoinBasicConfig::get_default_txn_dissem")]
    pub txn_dissem: DissemPattern,
    #[serde(default = "BitcoinBasicConfig::get_default_blk_dissem")]
    pub blk_dissem: DissemPattern,
}

impl Default for BitcoinBasicConfig {
    fn default() -> Self {
        Self {
            blk_size: 0x100000,
            difficulty: 25,
            commit_depth: 6,
            txn_dissem: DissemPattern::Gossip,
            blk_dissem: DissemPattern::Gossip,
        }
    }
}
