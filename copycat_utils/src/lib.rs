mod error;
pub use error::CopycatError;

pub mod log;

pub type NodeId = u64;

#[macro_use]
mod config;

mod parse_topo;
pub use parse_topo::get_neighbors;
