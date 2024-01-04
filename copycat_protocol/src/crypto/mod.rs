mod dummy;

use copycat_utils::CopycatError;

use ring::digest::{Context, SHA256};

use primitive_types::U256;

pub type Hash = U256;
pub type PrivKey = [u8; 32];
pub type PubKey = [u8; 32]; //TODO: 33 bytes but rust serde only support up to 32 bytes
pub type Signature = Vec<u8>;

#[derive(Copy, Clone, Debug, clap::ValueEnum)]
pub enum CryptoScheme {
    Dummy,
    // TODO: add ECDSA for bitcoin
}

impl CryptoScheme {
    pub fn gen_key_pair(&self, seed: u64) -> (PubKey, PrivKey) {
        match self {
            CryptoScheme::Dummy => dummy::gen_key_pair(seed),
        }
    }

    pub fn sign(&self, privkey: &PrivKey, input: &[u8]) -> Result<Signature, CopycatError> {
        match self {
            CryptoScheme::Dummy => dummy::sign(privkey, input),
        }
    }

    pub fn verify(
        &self,
        pubkey: &PubKey,
        input: &[u8],
        signature: &Signature,
    ) -> Result<bool, CopycatError> {
        match self {
            CryptoScheme::Dummy => dummy::verify(pubkey, input, signature),
        }
    }
}

pub fn sha256(input: &[u8]) -> Result<Hash, CopycatError> {
    let mut context = Context::new(&SHA256);
    context.update(input);
    let digest = context.finish();
    Ok(U256::from_little_endian(digest.as_ref()))
}
