use super::Hash;
use crate::CopycatError;

use rand::Rng;
use tokio::time::Duration;

// TODO: add Authenticated Multipoint evaluation Tree (AMT)

#[derive(Clone, Debug)]
pub struct DummyMerkleTree {
    root: Hash,
}

impl DummyMerkleTree {
    const TIMEOUT_PER_LEAF: f64 = 1.23e-5; // from experiment with rs-merkle crate

    pub fn new(num_elem: usize) -> Result<(Self, Duration), CopycatError> {
        let timeout = Duration::from_secs_f64(Self::TIMEOUT_PER_LEAF * num_elem as f64);
        let mut rng = rand::thread_rng();
        let raw: [u8; 32] = rng.gen();
        let tree = Self {
            root: Hash::from(raw),
        };
        Ok((tree, timeout))
    }

    pub fn get_root(&self) -> Hash {
        self.root
    }

    pub fn verify(_root: &Hash, num_elem: usize) -> Result<(bool, Duration), CopycatError> {
        let timeout = Duration::from_secs_f64(Self::TIMEOUT_PER_LEAF * num_elem as f64);
        Ok((true, timeout))
    }
}
