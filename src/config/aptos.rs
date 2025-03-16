use crate::DissemPattern;

use default_fields::DefaultFields;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AptosConfig {
    Basic { config: AptosDiemConfig },
}

#[derive(Debug, Clone, Serialize, Deserialize, DefaultFields)]
pub struct AptosDiemConfig {
    #[serde(default = "AptosDiemConfig::get_default_narwhal_blk_size")]
    pub narwhal_blk_size: usize,
    #[serde(default = "AptosDiemConfig::get_default_diem_blk_len")]
    pub diem_blk_len: usize,
    #[serde(default = "AptosDiemConfig::get_default_proposal_timeout_secs")]
    pub proposal_timeout_secs: f64,
    #[serde(default = "AptosDiemConfig::get_default_vote_timeout_secs")]
    pub vote_timeout_secs: f64,
    #[serde(default = "AptosDiemConfig::get_default_txn_dissem")]
    pub txn_dissem: DissemPattern,
}

impl Default for AptosDiemConfig {
    fn default() -> Self {
        Self {
            narwhal_blk_size: 0x100000, // https://github.com/aptos-labs/aptos-core/blob/308d59ec2e7d9c3937c8b6b4fca6dd7e97fd3196/config/src/config/quorum_store_config.rs#L114
            diem_blk_len: 28, // 7000 txns/block / 250 txns/batch: https://github.com/aptos-labs/aptos-core/blob/308d59ec2e7d9c3937c8b6b4fca6dd7e97fd3196/config/src/config/quorum_store_config.rs#L112
            proposal_timeout_secs: 5.0,
            vote_timeout_secs: 5.0,
            txn_dissem: DissemPattern::Broadcast,
        }
    }
}
