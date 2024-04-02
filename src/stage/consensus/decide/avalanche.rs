use super::Decision;
use crate::config::AvalancheConfig;
use crate::peers::PeerMessenger;
use crate::protocol::block::{Block, BlockHeader};
use crate::protocol::crypto::{sha256, Hash, PubKey};
use crate::utils::CopycatError;
use crate::NodeId;

use async_trait::async_trait;
use tokio::sync::Notify;

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

struct ConflictSet {
    pub pref: Hash,
    pub last_pref: Hash,
    pub count: u64,
}

pub struct AvalancheDecision {
    id: NodeId,
    vote_thresh: f64,
    beta1: usize,
    beta2: usize,
    block_pool: HashMap<Hash, Arc<Block>>,
    dag: HashMap<Hash, Vec<Hash>>,
    conflict_sets: HashMap<(Hash, PubKey), ConflictSet>,
    confidence: HashMap<Hash, (bool, u64)>, // (has_chit, confidence)
    // blocks ready to be committed
    commit_queue: VecDeque<Hash>,
    commit_count: u64,
    notify: Notify,
    peer_messenger: Arc<PeerMessenger>,
}

impl AvalancheDecision {
    pub fn new(id: NodeId, config: AvalancheConfig, peer_messenger: Arc<PeerMessenger>) -> Self {
        Self {
            id,
            vote_thresh: config.k as f64 * config.alpha,
            beta1: config.beta1,
            beta2: config.beta2,
            block_pool: HashMap::new(),
            dag: HashMap::new(),
            conflict_sets: HashMap::new(),
            confidence: HashMap::new(),
            commit_queue: VecDeque::new(),
            commit_count: 0,
            notify: Notify::new(),
            peer_messenger,
        }
    }
}

#[async_trait]
impl Decision for AvalancheDecision {
    async fn new_tail(&mut self, mut new_tail: Vec<Arc<Block>>) -> Result<(), CopycatError> {
        let new_blk = if new_tail.len() < 1 {
            return Ok(());
        } else if new_tail.len() == 1 {
            new_tail.remove(0)
        } else {
            unreachable!("Avalanche blocks are in DAG not chain")
        };

        let blk_hash = sha256(&bincode::serialize(&new_blk.header)?)?;
        self.block_pool.insert(blk_hash, new_blk.clone());

        let proposer = match &new_blk.header {
            BlockHeader::Avalanche { proposer, .. } => proposer,
            _ => unreachable!(),
        };

        // check if new_blk is preferred
        // if so, send vote message to others
        todo!();
    }

    async fn commit_ready(&self) -> Result<(), CopycatError> {
        loop {
            if self.commit_queue.len() > 0 {
                return Ok(());
            }
            self.notify.notified();
        }
    }

    async fn next_to_commit(&mut self) -> Result<(u64, Arc<Block>), CopycatError> {
        let blk_hash = self.commit_queue.pop_front().unwrap();
        let blk = self.block_pool.get(&blk_hash).unwrap();
        self.commit_count += 1;
        return Ok((self.commit_count, blk.clone()));
    }

    async fn handle_peer_msg(
        &mut self,
        src: NodeId,
        content: Arc<Vec<u8>>,
    ) -> Result<(), CopycatError> {
        todo!();
    }
}
