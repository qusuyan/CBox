use tokio::time::{Duration, Instant};

use super::{PrivKey, PubKey, Signature};
use crate::utils::CopycatError;

// ECDSA keys are 64 bytes
pub fn gen_key_pair(seed: u64) -> (PubKey, PrivKey) {
    let mut pubkey = [0u8; 32];
    pubkey[..8].clone_from_slice(&seed.to_le_bytes());
    let privkey = pubkey.clone();
    (pubkey, privkey)
}

pub async fn sign(_privkey: &PrivKey, input: &[u8]) -> Result<Signature, CopycatError> {
    const SIGN_TIME: f64 = 0.241e-3; // measured with k256 for messages < 1KB
    const HASH_TIME_PER_BYTE: f64 = 5.24e-9;
    let start = Instant::now();
    let sign_time = HASH_TIME_PER_BYTE * input.len() as f64 + SIGN_TIME;
    log::info!("signing {} bytes take {} secs", input.len(), sign_time);
    tokio::time::sleep_until(start + Duration::from_secs_f64(sign_time)).await;
    Ok(vec![0; 64])
}

pub async fn verify(
    _pubkey: &PubKey,
    input: &[u8],
    _signature: &Signature,
) -> Result<bool, CopycatError> {
    const VERIFY_TIME: f64 = 0.4e-3; // measured with k256 for messages < 1KB
    const HASH_TIME_PER_BYTE: f64 = 5.24e-9;
    let start = Instant::now();
    let verify_time = HASH_TIME_PER_BYTE * input.len() as f64 + VERIFY_TIME;
    log::info!("verifying {} bytes take {} secs", input.len(), verify_time);
    tokio::time::sleep_until(start + Duration::from_secs_f64(verify_time)).await;
    Ok(true)
}
