mod dummy;
mod dummy_ecdsa;
mod ecdsa;

use super::{PrivKey, PubKey, Signature};
use crate::CopycatError;

#[derive(Copy, Clone, Debug, clap::ValueEnum)]
pub enum SignatureScheme {
    Dummy,
    DummyECDSA,
    ECDSA,
}

impl SignatureScheme {
    pub fn gen_key_pair(&self, seed: u64) -> (PubKey, PrivKey) {
        match self {
            SignatureScheme::Dummy => dummy::gen_key_pair(seed),
            SignatureScheme::DummyECDSA => dummy_ecdsa::gen_key_pair(seed),
            SignatureScheme::ECDSA => ecdsa::gen_key_pair(seed),
        }
    }

    pub fn sign(&self, privkey: &PrivKey, input: &[u8]) -> Result<(Signature, f64), CopycatError> {
        match self {
            SignatureScheme::Dummy => dummy::sign(privkey, input),
            SignatureScheme::DummyECDSA => dummy_ecdsa::sign(privkey, input),
            SignatureScheme::ECDSA => ecdsa::sign(privkey, input),
        }
    }

    pub fn verify(
        &self,
        pubkey: &PubKey,
        input: &[u8],
        signature: &Signature,
    ) -> Result<(bool, f64), CopycatError> {
        match self {
            SignatureScheme::Dummy => dummy::verify(pubkey, input, signature),
            SignatureScheme::DummyECDSA => dummy_ecdsa::verify(pubkey, input, signature),
            SignatureScheme::ECDSA => ecdsa::verify(pubkey, input, signature),
        }
    }
}
