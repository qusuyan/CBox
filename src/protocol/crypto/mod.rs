pub mod signature;
pub mod threshold_signature;
pub mod vector_snark;

use crate::utils::CopycatError;

use std::fmt::Display;

use get_size::GetSize;
use primitive_types::U256;
use ring::digest::{Context, SHA256};
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct Hash(pub U256);

// TODO: Pubkey, Privkey, and Signature have fixed length
pub type PrivKey = Vec<u8>;
pub type PubKey = Vec<u8>;
pub type Signature = Vec<u8>;

pub fn sha256<T: Serialize>(input: &T) -> Result<Hash, CopycatError> {
    let serialized = bincode::serialize(input)?;
    let mut context = Context::new(&SHA256);
    context.update(&serialized);
    let digest = context.finish();
    Ok(Hash(U256::from_little_endian(digest.as_ref())))
}

impl GetSize for Hash {
    fn get_size(&self) -> usize {
        32
    }
}

impl Display for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::hash::Hash for Hash {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}
