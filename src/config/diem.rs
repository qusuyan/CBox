use crate::DissemPattern;

use default_fields::DefaultFields;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DiemConfig {
    Basic { config: DiemBasicConfig },
}

#[derive(Clone, Debug, Serialize, Deserialize, DefaultFields)]
pub struct DiemBasicConfig {
    #[serde(default = "DiemBasicConfig::get_default_blk_size")]
    pub blk_size: usize,
    #[serde(default = "DiemBasicConfig::get_default_proposal_timeout_secs")]
    pub proposal_timeout_secs: f64,
    #[serde(default = "DiemBasicConfig::get_default_vote_timeout_secs")]
    pub vote_timeout_secs: f64,
    #[serde(default = "DiemBasicConfig::get_default_txn_dissem")]
    pub txn_dissem: DissemPattern,
}

impl Default for DiemBasicConfig {
    fn default() -> Self {
        Self {
            blk_size: 0x100000,
            proposal_timeout_secs: 5.0,
            vote_timeout_secs: 5.0,
            txn_dissem: DissemPattern::Broadcast,
        }
    }
}
