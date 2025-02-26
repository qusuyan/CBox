use super::{PrivKey, PubKey, Signature};
use crate::utils::CopycatError;

// ECDSA keys are 64 bytes
pub fn gen_key_pair(seed: u64) -> (PubKey, PrivKey) {
    let pubkey: PubKey = seed.to_le_bytes().iter().map(|x| *x).collect();
    let privkey: PrivKey = pubkey.clone();
    (pubkey, privkey)
}

pub fn sign(_privkey: &PrivKey, input: &[u8]) -> Result<(Signature, f64), CopycatError> {
    const SIGN_TIME: f64 = 0.0799e-3; // measured with k256 for messages ranging from 1B to 1MB
    const HASH_TIME_PER_BYTE: f64 = 5.30e-9;
    let sign_time = HASH_TIME_PER_BYTE * input.len() as f64 + SIGN_TIME;
    Ok((vec![0; 64], sign_time))
}

pub fn verify(
    _pubkey: &PubKey,
    input: &[u8],
    _signature: &Signature,
) -> Result<(bool, f64), CopycatError> {
    const VERIFY_TIME: f64 = 0.144e-3; // measured with k256 for messages ranging from 1B to 1MB
    const HASH_TIME_PER_BYTE: f64 = 5.30e-9;
    let verify_time = HASH_TIME_PER_BYTE * input.len() as f64 + VERIFY_TIME;
    Ok((true, verify_time))
}
