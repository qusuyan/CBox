use crate::protocol::crypto::threshold_signature::{SignComb, ThresholdSignature};
use crate::protocol::crypto::Hash;
use crate::{CopycatError, NodeId};

use mailbox_client::{MailboxError, SizedMsg};
use serde::{Deserialize, Serialize};

// certificate of availability
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CoA {
    pub sender: NodeId,
    pub round: u64,
    pub digest: Hash,
    pub signature: SignComb,
}

impl CoA {
    pub fn validate(&self, comb_key: &dyn ThresholdSignature) -> Result<(bool, f64), CopycatError> {
        let content = (&self.sender, &self.round, &self.digest);
        let serialized = bincode::serialize(&content)?;
        comb_key.verify(&serialized, &self.signature)
    }
}

impl SizedMsg for CoA {
    fn size(&self) -> Result<usize, MailboxError> {
        let size = 8 + 8 + 32 + self.signature.len();
        Ok(size)
    }
}
