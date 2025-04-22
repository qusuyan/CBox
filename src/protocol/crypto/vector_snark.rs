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
    const TIMEOUT_PER_NODE: f64 = 1.23e-5; // from experiment with rs-merkle crate

    pub fn new(num_elem: usize) -> (Self, f64) {
        (
            Self {
                root: Hash(U256::zero()),
                num_elem,
            },
            Self::TIMEOUT_PER_NODE * num_elem as f64,
        )
    }

    pub fn append(&mut self, num_elem: usize) -> Result<f64, CopycatError> {
        let num_internal = if self.num_elem == 0 {
            0f64
        } else {
            (self.num_elem as f64).log2()
        };
        let timeout = Self::TIMEOUT_PER_NODE * num_internal * num_elem as f64;
        let mut rng = rand::thread_rng();
        let raw: [u8; 32] = rng.gen();
        self.root = Hash(U256::from(raw));
        self.num_elem += num_elem;
        Ok(timeout)
    }

    pub fn update(&mut self, num_elem: usize) -> Result<f64, CopycatError> {
        let num_internal = if self.num_elem == 0 {
            0f64
        } else {
            (self.num_elem as f64).log2()
        };
        let timeout = Self::TIMEOUT_PER_NODE * num_internal * num_elem as f64;
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
        let (_, dur) = DummyMerkleTree::new(num_elem);
        Ok((true, dur))
    }
}
