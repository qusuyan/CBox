use crate::DissemPattern;

use default_fields::DefaultFields;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, DefaultFields)]
pub struct DummyConfig {
    #[serde(default = "DummyConfig::get_default_txn_dissem")]
    pub txn_dissem: DissemPattern,
    #[serde(default = "DummyConfig::get_default_blk_dissem")]
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
