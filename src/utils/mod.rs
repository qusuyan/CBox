mod error;
pub use error::CopycatError;

pub mod log;

#[macro_use]
pub mod parse_config;

mod parse_topo;
pub use parse_topo::get_neighbors;

pub type NodeId = u64;
