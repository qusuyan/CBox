use crate::protocol::crypto::{Hash, PubKey, Signature};
use crate::{CopycatError, SignatureScheme};

use mailbox_client::{MailboxError, SizedMsg};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DummyTxn {
    pub id: Hash,
    pub content: Arc<Vec<u8>>,
    pub pub_key: PubKey,
    pub signature: Signature,
}

impl DummyTxn {
    pub fn compute_id(&self) -> Result<Hash, CopycatError> {
        Ok(self.id)
    }

    pub fn validate(&self, crypto: SignatureScheme) -> Result<(bool, f64), CopycatError> {
        crypto.verify(&self.pub_key, &self.content, &self.signature)
    }
}

impl SizedMsg for DummyTxn {
    fn size(&self) -> Result<usize, MailboxError> {
        Ok(32 + self.content.size()? + 32 + self.signature.len())
    }
}
