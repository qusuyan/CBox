mod error;
pub use error::CopycatError;

pub mod log;

pub mod parse_config;

mod parse_topo;
pub use parse_topo::{get_neighbors, get_topology};

pub type NodeId = u64;
