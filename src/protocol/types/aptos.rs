use crate::protocol::crypto::threshold_signature::{SignComb, ThresholdSignature};
use crate::protocol::crypto::Hash;
use crate::{CopycatError, NodeId};

use get_size::GetSize;
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
        let content = (self.sender, self.round, self.digest);
        let serialized = bincode::serialize(&content)?;
        comb_key.verify(&serialized, &self.signature)
    }
}

impl GetSize for CoA {
    fn get_size(&self) -> usize {
        48 + self.signature.len()
    }
}
