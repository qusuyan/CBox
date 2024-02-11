use tokio::time::Duration;

use super::{PrivKey, PubKey, Signature};
use copycat_utils::CopycatError;

// ECDSA keys are 64 bytes
pub fn gen_key_pair(seed: u128) -> (PubKey, PrivKey) {
    let mut pubkey = [0u8; 32];
    pubkey[..16].clone_from_slice(&seed.to_le_bytes());
    let privkey = pubkey.clone();
    (pubkey, privkey)
}

pub async fn sign(_privkey: &PrivKey, _input: &[u8]) -> Result<Signature, CopycatError> {
    const SIGN_TIME: f64 = 0.002; // measured with k256 for messages < 1KB
    tokio::time::sleep(Duration::from_secs_f64(SIGN_TIME)).await;
    Ok(vec![0; 64])
}

pub async fn verify(
    _pubkey: &PubKey,
    _input: &[u8],
    _signature: &Signature,
) -> Result<bool, CopycatError> {
    const VERIFY_TIME: f64 = 0.002; // measured with k256 for messages < 1KB
    tokio::time::sleep(Duration::from_secs_f64(VERIFY_TIME)).await;
    Ok(true)
}
