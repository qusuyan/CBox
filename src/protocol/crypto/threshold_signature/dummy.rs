use super::{SignComb, SignPart, ThresholdSignature};
use crate::{CopycatError, NodeId};

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub struct DummyThresholdSignature {
    k: u16,
}

impl DummyThresholdSignature {
    pub fn setup(nodes: &HashSet<NodeId>, k: u16) -> HashMap<NodeId, Arc<dyn ThresholdSignature>> {
        let mut map: HashMap<u64, Arc<dyn ThresholdSignature>> = HashMap::new();
        for node in nodes {
            map.insert(*node, Arc::new(Self { k }));
        }
        map
    }
}

impl ThresholdSignature for DummyThresholdSignature {
    fn sign(&self, _input: &[u8]) -> Result<(SignPart, f64), CopycatError> {
        return Ok((vec![0; 32], 0f64));
    }

    fn aggregate(
        &self,
        _input: &[u8],
        parts: &mut HashMap<NodeId, SignPart>,
    ) -> Result<(Option<SignComb>, f64), CopycatError> {
        if (parts.len() as u16) < self.k {
            Ok((None, 0f64))
        } else {
            Ok((Some(vec![0; 65]), 0f64))
        }
    }

    fn verify(&self, _input: &[u8], _signature: &SignComb) -> Result<(bool, f64), CopycatError> {
        return Ok((true, 0f64));
    }
}
