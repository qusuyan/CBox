mod basic;
use basic::AptosPmaker;

use super::Pacemaker;
use crate::config::AptosConfig;
use crate::NodeId;

pub fn new(id: NodeId, config: AptosConfig) -> Box<dyn Pacemaker> {
    match config {
        AptosConfig::Basic { config } => Box::new(AptosPmaker::new(id, config)),
    }
}
