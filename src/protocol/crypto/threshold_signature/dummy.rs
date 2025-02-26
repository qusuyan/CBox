use super::{SignComb, SignPart, ThresholdSignature};
use crate::CopycatError;

pub struct DummyThresholdSignature;

impl DummyThresholdSignature {
    pub fn new() -> Self {
        Self
    }
}

impl ThresholdSignature for DummyThresholdSignature {
    fn sign(&self, _input: &[u8]) -> Result<SignPart, CopycatError> {
        return Ok(vec![0; 32]);
    }

    fn aggregate(
        &self,
        _input: &[u8],
        _parts: &mut std::collections::HashMap<crate::NodeId, SignPart>,
    ) -> Result<SignComb, CopycatError> {
        return Ok(vec![0; 65]);
    }

    fn verify(&self, _input: &[u8], _signature: &SignComb) -> Result<(), CopycatError> {
        return Ok(());
    }
}
