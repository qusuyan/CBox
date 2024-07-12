use crate::{
    protocol::crypto::{Hash, PubKey, Signature},
    CopycatError, CryptoScheme,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DummyTxn {
    pub id: Hash,
    pub content: Vec<u8>,
    pub pub_key: PubKey,
    pub signature: Signature,
}

impl DummyTxn {
    pub fn get_size(&self) -> usize {
        self.content.len() + self.signature.len() + 64
    }

    pub fn validate(&self, crypto: CryptoScheme) -> Result<(bool, f64), CopycatError> {
        crypto.verify(&self.pub_key, &self.content, &self.signature)
    }
}
