mod dummy;
mod dummy_ecdsa;

use tokio::time::Duration;

use crate::utils::CopycatError;

use rand::Rng;
use ring::digest::{Context, SHA256};

use primitive_types::U256;

pub type Hash = U256;
pub type PrivKey = [u8; 32];
pub type PubKey = [u8; 32]; //TODO: 33 bytes but rust serde only support up to 32 bytes
pub type Signature = Vec<u8>;

#[derive(Copy, Clone, Debug, clap::ValueEnum)]
pub enum CryptoScheme {
    Dummy,
    DummyECDSA,
    // TODO: add ECDSA for bitcoin
}

impl CryptoScheme {
    pub fn gen_key_pair(&self, seed: u128) -> (PubKey, PrivKey) {
        match self {
            CryptoScheme::Dummy => dummy::gen_key_pair(seed),
            CryptoScheme::DummyECDSA => dummy_ecdsa::gen_key_pair(seed),
        }
    }

    pub async fn sign(&self, privkey: &PrivKey, input: &[u8]) -> Result<Signature, CopycatError> {
        match self {
            CryptoScheme::Dummy => dummy::sign(privkey, input),
            CryptoScheme::DummyECDSA => dummy_ecdsa::sign(privkey, input).await,
        }
    }

    pub async fn verify(
        &self,
        pubkey: &PubKey,
        input: &[u8],
        signature: &Signature,
    ) -> Result<bool, CopycatError> {
        match self {
            CryptoScheme::Dummy => dummy::verify(pubkey, input, signature),
            CryptoScheme::DummyECDSA => dummy_ecdsa::verify(pubkey, input, signature).await,
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
pub struct DummyMerkleTree(pub Hash);

impl DummyMerkleTree {
    const TIMEOUT_PER_LEAF: f64 = 1.23e-5; // from experiment with rs-merkle crate

    pub async fn new(num_elem: usize) -> Result<Self, CopycatError> {
        tokio::time::sleep(Duration::from_secs_f64(
            Self::TIMEOUT_PER_LEAF * num_elem as f64,
        ))
        .await;
        let mut rng = rand::thread_rng();
        let raw: [u8; 32] = rng.gen();
        Ok(Self(U256::from(raw)))
    }

    pub async fn verify(self, num_elem: usize) -> Result<bool, CopycatError> {
        tokio::time::sleep(Duration::from_secs_f64(
            Self::TIMEOUT_PER_LEAF * num_elem as f64,
        ))
        .await;
        Ok(true)
    }
}
