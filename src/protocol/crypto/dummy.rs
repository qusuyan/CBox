use super::{PrivKey, PubKey, Signature};
use crate::utils::CopycatError;

pub fn gen_key_pair(seed: u64) -> (PubKey, PrivKey) {
    let mut pubkey = [0u8; 32];
    pubkey[..8].clone_from_slice(&seed.to_le_bytes());
    let privkey = pubkey.clone();
    (pubkey, privkey)
}

pub fn sign(_privkey: &PrivKey, _input: &[u8]) -> Result<Signature, CopycatError> {
    Ok(vec![0; 64])
}

pub fn verify(
    _pubkey: &PubKey,
    _input: &[u8],
    _signature: &Signature,
) -> Result<bool, CopycatError> {
    Ok(true)
}
