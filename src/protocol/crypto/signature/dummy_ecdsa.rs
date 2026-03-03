use super::{PrivKey, PubKey, Signature};
use crate::utils::CopycatError;

const KEY_LENGTH: usize = 32; // ECDSA keys are 32 bytes
const SIGNATURE_LENGTH: usize = 64; // ECDSA signatures are 64 bytes

pub fn gen_key_pair(seed: u64) -> (PubKey, PrivKey) {
    let mut pubkey = seed.to_le_bytes().to_vec();
    pubkey.resize(KEY_LENGTH, 0);
    let privkey: PrivKey = pubkey.clone();
    (pubkey, privkey)
}

pub fn sign(_privkey: &PrivKey, input: &[u8]) -> Result<(Signature, f64), CopycatError> {
    const SIGN_TIME: f64 = 0.0799e-3; // measured with k256 for messages ranging from 1B to 1MB
    const HASH_TIME_PER_BYTE: f64 = 5.30e-9;
    let sign_time = HASH_TIME_PER_BYTE * input.len() as f64 + SIGN_TIME;
    Ok((vec![0; SIGNATURE_LENGTH], sign_time))
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

#[cfg(test)]
mod dummy_ecdsa_test {
    use super::*;

    use rand::{thread_rng, Rng};

    #[test]
    fn test_ecdsa_key_correct() {
        let mut rng = thread_rng();
        let seed: u64 = rng.gen();
        let (pubkey, _) = gen_key_pair(seed);
        for (idx, val) in seed.to_le_bytes().iter().enumerate() {
            assert_eq!(pubkey[idx], *val);
        }
    }

    #[test]
    fn test_ecdsa_key_size() {
        let (pubkey, privkey) = gen_key_pair(0);
        assert_eq!(pubkey.len(), KEY_LENGTH);
        assert_eq!(privkey.len(), KEY_LENGTH);
    }

    #[test]
    fn test_ecdsa_signature_size() {
        let (_, privkey) = gen_key_pair(0);
        let (signature, _) = sign(&privkey, &vec![1; 400]).unwrap();
        assert_eq!(signature.len(), SIGNATURE_LENGTH);
    }
}
