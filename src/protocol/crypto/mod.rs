mod dummy;
mod dummy_ecdsa;
mod ecdsa;

use tokio::time::Duration;

use crate::utils::CopycatError;

use rand::Rng;
use ring::digest::{Context, SHA256};

use primitive_types::U256;

pub type Hash = U256;
pub type PrivKey = Box<[u8]>;
pub type PubKey = Box<[u8]>; //TODO: 33 bytes but rust serde only support up to 32 bytes
pub type Signature = Vec<u8>;

#[derive(Copy, Clone, Debug, clap::ValueEnum)]
pub enum CryptoScheme {
    Dummy,
    DummyECDSA,
    ECDSA,
    // TODO: add ECDSA for bitcoin
}

impl CryptoScheme {
    pub fn gen_key_pair(&self, seed: u64) -> (PubKey, PrivKey) {
        match self {
            CryptoScheme::Dummy => dummy::gen_key_pair(seed),
            CryptoScheme::DummyECDSA => dummy_ecdsa::gen_key_pair(seed),
            CryptoScheme::ECDSA => ecdsa::gen_key_pair(seed),
        }
    }

    pub fn sign(&self, privkey: &PrivKey, input: &[u8]) -> Result<(Signature, f64), CopycatError> {
        match self {
            CryptoScheme::Dummy => dummy::sign(privkey, input),
            CryptoScheme::DummyECDSA => dummy_ecdsa::sign(privkey, input),
            CryptoScheme::ECDSA => ecdsa::sign(privkey, input),
        }
    }

    pub fn verify(
        &self,
        pubkey: &PubKey,
        input: &[u8],
        signature: &Signature,
    ) -> Result<(bool, f64), CopycatError> {
        match self {
            CryptoScheme::Dummy => dummy::verify(pubkey, input, signature),
            CryptoScheme::DummyECDSA => dummy_ecdsa::verify(pubkey, input, signature),
            CryptoScheme::ECDSA => ecdsa::verify(pubkey, input, signature),
        }
    }
}

pub fn sha256(input: &[u8]) -> Result<Hash, CopycatError> {
    let mut context = Context::new(&SHA256);
    context.update(input);
    let digest = context.finish();
    Ok(U256::from_little_endian(digest.as_ref()))
}

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
            root: U256::from(raw),
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
