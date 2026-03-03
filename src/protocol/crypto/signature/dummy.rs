use super::{PrivKey, PubKey, Signature};
use crate::utils::CopycatError;

pub fn gen_key_pair(seed: u64) -> (PubKey, PrivKey) {
    let pubkey: PubKey = seed.to_le_bytes().to_vec();
    let privkey: PrivKey = pubkey.clone();
    (pubkey, privkey)
}

pub fn sign(_privkey: &PrivKey, _input: &[u8]) -> Result<(Signature, f64), CopycatError> {
    Ok((vec![0; 64], 0f64))
}

pub fn verify(
    _pubkey: &PubKey,
    _input: &[u8],
    _signature: &Signature,
) -> Result<(bool, f64), CopycatError> {
    Ok((true, 0f64))
}
