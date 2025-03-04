mod dummy;
mod ecdsa;
mod frost;

use crate::{CopycatError, NodeId};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub type SignPart = Vec<u8>; // Fixed length U256
pub type SignComb = Vec<u8>; // this may be variable length due to implementation

#[derive(Copy, Clone, Debug, clap::ValueEnum)]
pub enum ThresholdSignatureScheme {
    Dummy,
    FROST,
    ECDSA,
}

impl ThresholdSignatureScheme {
    pub fn to_threshold_signature(
        &self,
        nodes: &HashSet<NodeId>,
        k: u16,
        seed: u64,
    ) -> Result<HashMap<NodeId, Arc<dyn ThresholdSignature>>, CopycatError> {
        match self {
            ThresholdSignatureScheme::Dummy => Ok(dummy::DummyThresholdSignature::setup(nodes, k)),
            ThresholdSignatureScheme::FROST => {
                Ok(frost::FrostThresholdSignature::setup(nodes, k, seed)?)
            }
            ThresholdSignatureScheme::ECDSA => {
                Ok(ecdsa::ECDSAThresholdSignature::setup(nodes, k, seed)?)
            }
        }
    }
}

pub trait ThresholdSignature: Sync + Send {
    fn sign(&self, input: &[u8]) -> Result<(SignPart, f64), CopycatError>;
    fn aggregate(
        &self,
        input: &[u8],
        parts: &mut HashMap<NodeId, SignPart>,
    ) -> Result<(Option<SignComb>, f64), CopycatError>;
    fn verify(&self, input: &[u8], signature: &SignComb) -> Result<(bool, f64), CopycatError>;
}
