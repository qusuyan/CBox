use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::BufReader;

use crate::utils::{CopycatError, NodeId};

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

pub fn get_topology(
    local_nodes: &Vec<NodeId>,
    path: String,
) -> Result<HashMap<NodeId, HashSet<NodeId>>, CopycatError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let pipes: Pipes = serde_json::from_reader(reader)?;

    let mut neighbor_map = HashMap::new();
    for node in local_nodes {
        neighbor_map.insert(*node, HashSet::new());
    }

    for (src, dst) in pipes {
        if let Some(neighbors) = neighbor_map.get_mut(&src) {
            neighbors.insert(dst);
        }
        if let Some(neighbors) = neighbor_map.get_mut(&dst) {
            neighbors.insert(src);
        }
    }

    Ok(neighbor_map)
}

pub fn fully_connected_topology(
    local_nodes: &Vec<NodeId>,
    all_nodes: &HashSet<NodeId>,
) -> HashMap<NodeId, HashSet<NodeId>> {
    let mut neighbor_map = HashMap::new();
    for node in local_nodes {
        let neighbors = all_nodes
            .iter()
            .filter(|neighbor| **neighbor != *node)
            .cloned()
            .collect();
        neighbor_map.insert(*node, neighbors);
    }
    neighbor_map
}
