mod error;
pub use error::CopycatError;

#[macro_use]
pub mod log;

pub mod parse_config;

mod parse_topo;
pub use parse_topo::{fully_connected_topology, get_neighbors, get_topology};

pub type NodeId = u64;
