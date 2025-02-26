use super::{PrivKey, PubKey, Signature};
use crate::utils::CopycatError;

use rand::prelude::*;
use rand_chacha::ChaCha8Rng;

use k256::ecdsa::{
    signature::{Signer, Verifier},
    SigningKey, VerifyingKey,
};

// ECDSA keys are 64 bytes
pub fn gen_key_pair(seed: u64) -> (PubKey, PrivKey) {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let signing_key = SigningKey::random(&mut rng);
    let verifying_key = VerifyingKey::from(&signing_key);
    let sk_bytes: PrivKey = signing_key.to_bytes().iter().map(|x| *x).collect();
    let vk_bytes: PubKey = verifying_key.to_sec1_bytes().to_vec();
    (vk_bytes, sk_bytes)
}

pub fn sign(privkey: &PrivKey, input: &[u8]) -> Result<(Signature, f64), CopycatError> {
    let signing_key = SigningKey::from_bytes(privkey.as_slice().into())?;
    let signature: k256::ecdsa::Signature = signing_key.sign(input);
    Ok((signature.to_vec(), 0f64))
}

pub fn verify(
    pubkey: &PubKey,
    input: &[u8],
    signature: &Signature,
) -> Result<(bool, f64), CopycatError> {
    let verifying_key = VerifyingKey::from_sec1_bytes(&pubkey)?;
    let signature: k256::ecdsa::Signature = k256::ecdsa::Signature::from_slice(signature)?;
    let valid = verifying_key.verify(input, &signature).is_ok();
    Ok((valid, 0f64))
}
