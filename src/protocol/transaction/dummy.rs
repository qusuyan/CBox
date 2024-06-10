use crate::protocol::crypto::Hash;

use get_size::GetSize;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DummyTxn {
    pub id: Hash,
    pub content: Vec<u8>,
}

impl DummyTxn {
    pub fn get_size(&self) -> usize {
        self.content.get_size() + 64
    }
}
