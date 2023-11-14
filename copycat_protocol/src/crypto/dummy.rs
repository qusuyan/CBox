use super::{PrivKey, PubKey, Signature};
use copycat_utils::CopycatError;

pub fn sign(_privkey: &PrivKey, input: &[u8]) -> Result<Signature, CopycatError> {
    Ok(Vec::from(input))
}

pub fn verify(
    _pubkey: &PubKey,
    _input: &[u8],
    _signature: &Signature,
) -> Result<bool, CopycatError> {
    Ok(true)
}
