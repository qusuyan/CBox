use super::{PrivKey, PubKey, Signature};
use crate::utils::CopycatError;

// ECDSA keys are 64 bytes
pub fn gen_key_pair(_seed: u64) -> (PubKey, PrivKey) {
    todo!();
    // let rng = rand::thread_rng();
    // let sk = ecdsa::SigningKey::random(&mut rng);
    // let sk = ecdsa::SigningKey::from_bytes(&seed.to_le_bytes());
}

pub fn sign(_privkey: &PrivKey, _input: &[u8]) -> Result<Signature, CopycatError> {
    todo!();
}

pub fn verify(
    _pubkey: &PubKey,
    _input: &[u8],
    _signature: &Signature,
) -> Result<bool, CopycatError> {
    todo!();
}
