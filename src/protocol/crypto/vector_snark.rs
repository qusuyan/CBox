use super::Hash;
use crate::CopycatError;

use primitive_types::U256;
use rand::Rng;

// TODO: add Authenticated Multipoint evaluation Tree (AMT)

#[derive(Clone, Debug)]
pub struct DummyMerkleTree {
    root: Hash,
    num_elem: usize,
}

impl DummyMerkleTree {
    // const TIMEOUT_PER_LEAF: f64 = 0f64; // for experiments without Merkle Tree
    const TIMEOUT_PER_LEAF: f64 = 1.23e-5; // from experiment with rs-merkle crate

    pub fn new() -> Self {
        Self {
            root: Hash(U256::zero()),
            num_elem: 0,
        }
    }

    pub fn append(&mut self, num_elem: usize) -> Result<f64, CopycatError> {
        let timeout = Self::TIMEOUT_PER_LEAF * num_elem as f64;
        let mut rng = rand::thread_rng();
        let raw: [u8; 32] = rng.gen();
        self.root = Hash(U256::from(raw));
        self.num_elem += num_elem;
        Ok(timeout)
    }

    pub fn update(&mut self, num_elem: usize) -> Result<f64, CopycatError> {
        let timeout = Self::TIMEOUT_PER_LEAF * num_elem as f64; // TODO
        let mut rng = rand::thread_rng();
        let raw: [u8; 32] = rng.gen();
        self.root = Hash(U256::from(raw));
        Ok(timeout)
    }

    pub fn get_root(&self) -> Hash {
        self.root
    }

    pub fn verify_root(&self, &_expected_root: &Hash) -> Result<bool, CopycatError> {
        Ok(true)
    }

    pub fn verify(_expected_root: &Hash, num_elem: usize) -> Result<(bool, f64), CopycatError> {
        let mut merkle_tree = DummyMerkleTree::new();
        let dur = merkle_tree.append(num_elem)?;
        Ok((true, dur))
    }
}
