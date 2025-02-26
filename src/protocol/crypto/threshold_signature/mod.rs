mod dummy;
mod ecdsa;
mod frost;

use crate::{CopycatError, NodeId};
use std::collections::HashMap;

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
        me: NodeId,
        nodes: Vec<NodeId>,
        k: u16,
        seed: u64,
    ) -> Result<Box<dyn ThresholdSignature>, CopycatError> {
        match self {
            ThresholdSignatureScheme::Dummy => Ok(Box::new(dummy::DummyThresholdSignature::new())),
            ThresholdSignatureScheme::FROST => Ok(Box::new(frost::FrostThresholdSignature::new(
                me, nodes, k, seed,
            )?)),
            ThresholdSignatureScheme::ECDSA => Ok(Box::new(ecdsa::ECDSAThresholdSignature::new(
                me, nodes, k, seed,
            )?)),
        }
    }
}

pub trait ThresholdSignature {
    fn sign(&self, input: &[u8]) -> Result<SignPart, CopycatError>;
    fn aggregate(
        &self,
        input: &[u8],
        parts: &mut HashMap<NodeId, SignPart>,
    ) -> Result<SignComb, CopycatError>;
    fn verify(&self, input: &[u8], signature: &SignComb) -> Result<(), CopycatError>;
}
