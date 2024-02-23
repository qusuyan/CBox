use std::collections::HashSet;
use std::fs::File;
use std::io::BufReader;

use crate::{CopycatError, NodeId};

type Pipes = Vec<(NodeId, NodeId)>;

pub fn get_neighbors(me: NodeId, path: String) -> Result<HashSet<NodeId>, CopycatError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let pipes: Pipes = serde_json::from_reader(reader)?;
    let mut neighbors = HashSet::new();
    for (src, dst) in pipes {
        if src == me {
            neighbors.insert(dst);
        } else if dst == me {
            neighbors.insert(src);
        }
    }
    Ok(neighbors)
}
