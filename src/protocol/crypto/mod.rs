pub mod signature;
pub mod threshold_signature;
pub mod vector_snark;

use crate::utils::CopycatError;

use primitive_types::U256;
use ring::digest::{Context, SHA256};
use serde::Serialize;

// TODO: Pubkey, Privkey, and Signature have fixed length
pub type Hash = U256;
pub type PrivKey = Vec<u8>;
pub type PubKey = Vec<u8>; // TODO: 33 bytes but rust serde only support up to 32 bytes
pub type Signature = Vec<u8>;

pub fn sha256<T: Serialize>(input: &T) -> Result<Hash, CopycatError> {
    let serialized = bincode::serialize(input)?;
    let mut context = Context::new(&SHA256);
    context.update(&serialized);
    let digest = context.finish();
    Ok(Hash::from_little_endian(digest.as_ref()))
}
