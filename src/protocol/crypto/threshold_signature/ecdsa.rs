use super::{SignComb, SignPart, ThresholdSignature};
use crate::{CopycatError, NodeId};

use rand::prelude::*;
use rand_chacha::ChaCha8Rng;

use k256::ecdsa::{
    signature::{Signer, Verifier},
    SigningKey, VerifyingKey,
};

use std::collections::HashMap;

pub struct ECDSAThresholdSignature {
    me: NodeId,
    k: u16,
    pubkeys: HashMap<NodeId, VerifyingKey>,
    privkey: SigningKey,
}

impl ECDSAThresholdSignature {
    pub fn new(me: NodeId, nodes: Vec<NodeId>, k: u16, seed: u64) -> Result<Self, CopycatError> {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let mut pubkeys = HashMap::new();
        let mut privkeys = HashMap::new();
        for node in nodes {
            let signing_key = SigningKey::random(&mut rng);
            let verifying_key = VerifyingKey::from(&signing_key);
            privkeys.insert(node, signing_key);
            pubkeys.insert(node, verifying_key);
        }

        let privkey = privkeys.remove(&me).ok_or(CopycatError(String::from(
            "Current node ID not in the node ID list",
        )))?;

        Ok(Self {
            me,
            k,
            pubkeys,
            privkey,
        })
    }
}

impl ThresholdSignature for ECDSAThresholdSignature {
    fn sign(&self, input: &[u8]) -> Result<SignPart, CopycatError> {
        let signature: k256::ecdsa::Signature = self.privkey.sign(input);
        Ok(signature.to_vec())
    }

    fn aggregate(
        &self,
        input: &[u8],
        parts: &mut HashMap<NodeId, SignPart>,
    ) -> Result<SignComb, CopycatError> {
        let mut invalid_list = vec![];
        for (node, part) in parts.iter() {
            let pubkey = match self.pubkeys.get(node) {
                Some(key) => key,
                None => {
                    pf_warn!(self.me; "Drop invalid signature share from Node {}: unknown node", node);
                    invalid_list.push(*node);
                    continue;
                }
            };
            let signature: k256::ecdsa::Signature = k256::ecdsa::Signature::from_slice(part)?;
            if let Err(e) = pubkey.verify(input, &signature) {
                pf_warn!(self.me; "Drop invalid signature share from Node {}: {:?}", node, e);
                invalid_list.push(*node);
                continue;
            }
        }

        for node in invalid_list {
            parts.remove(&node);
        }

        if parts.len() < self.k as usize {
            Err(CopycatError(String::from("Not enough valid shares")))
        } else {
            Ok(bincode::serialize(parts)?)
        }
    }

    fn verify(&self, input: &[u8], signature: &SignComb) -> Result<(), CopycatError> {
        let parts: HashMap<NodeId, SignPart> = bincode::deserialize(&signature)?;
        if parts.len() < self.k as usize {
            return Err(CopycatError(String::from("Not enough valid shares")));
        }

        for (node, part) in parts.iter() {
            let pubkey = match self.pubkeys.get(node) {
                Some(key) => key,
                None => {
                    return Err(CopycatError(String::from("Unrecognized node")));
                }
            };
            let signature: k256::ecdsa::Signature = k256::ecdsa::Signature::from_slice(part)?;
            pubkey.verify(input, &signature)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod ecdsa_threshold_signature_test {

    use super::ECDSAThresholdSignature;
    use super::ThresholdSignature;
    use crate::CopycatError;

    use bincode;
    use std::collections::HashMap;

    #[test]
    fn test_ecdsa_threshold_signature() -> Result<(), CopycatError> {
        let message = bincode::serialize("test message content")?;

        let scheme0 = ECDSAThresholdSignature::new(0, vec![0, 1, 2, 3, 4], 3, 241241)?;
        let scheme2 = ECDSAThresholdSignature::new(2, vec![0, 1, 2, 3, 4], 3, 241241)?;
        let scheme3 = ECDSAThresholdSignature::new(3, vec![0, 1, 2, 3, 4], 3, 241241)?;

        let part0 = scheme0.sign(&message)?;
        let part2 = scheme2.sign(&message)?;
        let part3 = scheme3.sign(&message)?;

        let mut parts = HashMap::new();
        parts.insert(0, part0);
        parts.insert(2, part2);
        parts.insert(3, part3);

        let combined0 = scheme0.aggregate(&message, &mut parts)?;
        let combined2 = scheme2.aggregate(&message, &mut parts)?;
        let combined3 = scheme3.aggregate(&message, &mut parts)?;

        scheme0.verify(&message, &combined2)?;
        scheme2.verify(&message, &combined3)?;
        scheme3.verify(&message, &combined0)?;

        Ok(())
    }
}
