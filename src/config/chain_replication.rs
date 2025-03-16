use crate::NodeId;

use default_fields::DefaultFields;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, DefaultFields)]
pub struct ChainReplicationConfig {
    #[serde(default = "ChainReplicationConfig::get_default_order")]
    pub order: Vec<NodeId>,
    #[serde(default = "ChainReplicationConfig::get_default_blk_size")]
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
