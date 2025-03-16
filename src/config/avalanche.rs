use crate::DissemPattern;

use default_fields::DefaultFields;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AvalancheConfig {
    Basic { config: AvalancheBasicConfig },
    Blizzard { config: AvalancheBasicConfig }, //https://arxiv.org/pdf/2401.02811
    VoteNo { config: AvalancheVoteNoConfig },
}

#[derive(Clone, Debug, Serialize, Deserialize, DefaultFields)]
pub struct AvalancheBasicConfig {
    #[serde(default = "AvalancheBasicConfig::get_default_blk_size")]
    pub blk_size: usize,
    #[serde(default = "AvalancheBasicConfig::get_default_k")]
    pub k: usize,
    #[serde(default = "AvalancheBasicConfig::get_default_alpha")]
    pub alpha: f64,
    #[serde(default = "AvalancheBasicConfig::get_default_beta1")]
    pub beta1: u64,
    #[serde(default = "AvalancheBasicConfig::get_default_beta2")]
    pub beta2: u64,
    #[serde(default = "AvalancheBasicConfig::get_default_proposal_timeout_secs")]
    pub proposal_timeout_secs: f64,
    #[serde(default = "AvalancheBasicConfig::get_default_vote_timeout_secs")]
    pub vote_timeout_secs: f64,
    #[serde(default = "AvalancheBasicConfig::get_default_max_inflight_blk")]
    pub max_inflight_blk: usize,
    #[serde(default = "AvalancheBasicConfig::get_default_txn_dissem")]
    pub txn_dissem: DissemPattern,
}

// https://arxiv.org/pdf/1906.08936.pdf
impl Default for AvalancheBasicConfig {
    fn default() -> Self {
        Self {
            blk_size: 4480, // 40 txns/blk * 112 B per noop txn
            k: 10,
            alpha: 0.8,
            beta1: 11,
            beta2: 150,
            proposal_timeout_secs: 5.0,
            vote_timeout_secs: 5.0,
            max_inflight_blk: 235, // 40 * max_inflight_blk ~ 1MB   (bitcoin block size)
            txn_dissem: DissemPattern::Broadcast,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AvalancheVoteNoConfig {}
