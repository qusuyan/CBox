use super::{SignComb, SignPart, ThresholdSignature};
use crate::protocol::crypto::{PrivKey, PubKey};
use crate::{CopycatError, NodeId, SignatureScheme};

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub struct SignatureBasedThresholdSignature {
    me: NodeId,
    signature_scheme: SignatureScheme,
    k: u16,
    pubkeys: HashMap<NodeId, PubKey>,
    privkey: PrivKey,
}

impl SignatureBasedThresholdSignature {
    pub fn setup(
        nodes: &HashSet<NodeId>,
        signature_scheme: SignatureScheme,
        k: u16,
        seed: u64,
    ) -> Result<HashMap<NodeId, Arc<dyn ThresholdSignature>>, CopycatError> {
        // let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let mut pubkeys = HashMap::new();
        let mut privkeys = HashMap::new();
        for node in nodes {
            let (pk, sk) = signature_scheme.gen_key_pair(seed + *node as u64);
            privkeys.insert(*node, pk);
            pubkeys.insert(*node, sk);
        }

        let mut quorum: HashMap<u64, Arc<dyn ThresholdSignature>> = HashMap::new();
        for node in nodes {
            let privkey = privkeys.remove(node).unwrap();
            let scheme = Self {
                me: *node,
                signature_scheme,
                k,
                pubkeys: pubkeys.clone(),
                privkey,
            };
            quorum.insert(*node, Arc::new(scheme));
        }

        Ok(quorum)
    }
}

impl ThresholdSignature for SignatureBasedThresholdSignature {
    fn sign(&self, input: &[u8]) -> Result<(SignPart, f64), CopycatError> {
        self.signature_scheme.sign(&self.privkey, input)
    }

    fn aggregate(
        &self,
        input: &[u8],
        parts: &mut HashMap<NodeId, SignPart>,
    ) -> Result<(Option<SignComb>, f64), CopycatError> {
        let mut invalid_list = vec![];
        let mut verification_time = 0f64;
        for (node, signature) in parts.iter() {
            let pubkey = match self.pubkeys.get(node) {
                Some(key) => key,
                None => {
                    pf_warn!(self.me; "Drop invalid signature share from Node {}: unknown node", node);
                    invalid_list.push(*node);
                    continue;
                }
            };

            match self.signature_scheme.verify(&pubkey, input, &signature) {
                Ok((result, dur)) => {
                    verification_time += dur;
                    if !result {
                        pf_warn!(self.me; "Drop invalid signature share from Node {}: signature verification failed", node);
                        invalid_list.push(*node);
                        continue;
                    }
                }
                Err(e) => {
                    pf_warn!(self.me; "Drop invalid signature share from Node {}: {:?}", node, e);
                    invalid_list.push(*node);
                    continue;
                }
            }
        }

        for node in invalid_list {
            parts.remove(&node);
        }

        if parts.len() < self.k as usize {
            pf_warn!(self.me; "Not enough valid shares");
            Ok((None, verification_time))
        } else {
            Ok((Some(bincode::serialize(parts)?), verification_time))
        }
    }

    fn verify(&self, input: &[u8], signature: &SignComb) -> Result<(bool, f64), CopycatError> {
        let parts: HashMap<NodeId, SignPart> = bincode::deserialize(&signature)?;
        if parts.len() < self.k as usize {
            pf_warn!(self.me; "Not enough valid shares");
            return Ok((false, 0f64));
        }

        let mut verification_time = 0f64;
        for (node, signature) in parts.iter() {
            let pubkey = match self.pubkeys.get(node) {
                Some(key) => key,
                None => {
                    pf_warn!(self.me; "Invalid signature share from Node {}: unknown node", node);
                    return Ok((false, verification_time));
                }
            };

            match self.signature_scheme.verify(&pubkey, input, &signature) {
                Ok((result, dur)) => {
                    verification_time += dur;
                    if !result {
                        pf_warn!(self.me; "Drop invalid signature share from Node {}: signature verification failed", node);
                        return Ok((false, verification_time));
                    }
                }
                Err(e) => {
                    pf_warn!(self.me; "Invalid signature share from Node {}: {:?}", node, e);
                    return Ok((false, verification_time));
                }
            }
        }

        Ok((true, verification_time))
    }
}

#[cfg(test)]
mod ecdsa_threshold_signature_test {

    use super::SignatureBasedThresholdSignature;
    use crate::CopycatError;
    use crate::SignatureScheme;

    use bincode;
    use std::collections::{HashMap, HashSet};

    #[test]
    fn test_ecdsa_threshold_signature() -> Result<(), CopycatError> {
        let message = bincode::serialize("test message content")?;

        let nodes = HashSet::from_iter(0..5);
        let mut schemes =
            SignatureBasedThresholdSignature::setup(&nodes, SignatureScheme::ECDSA, 3, 241241)?;
        let scheme0 = schemes.remove(&0).unwrap();
        let scheme2 = schemes.remove(&2).unwrap();
        let scheme3 = schemes.remove(&3).unwrap();

        let (part0, _) = scheme0.sign(&message)?;
        let (part2, _) = scheme2.sign(&message)?;
        let (part3, _) = scheme3.sign(&message)?;

        let mut parts = HashMap::new();
        parts.insert(0, part0);
        parts.insert(2, part2);
        parts.insert(3, part3);

        let (combined0, _) = scheme0.aggregate(&message, &mut parts)?;
        let (combined2, _) = scheme2.aggregate(&message, &mut parts)?;
        let (combined3, _) = scheme3.aggregate(&message, &mut parts)?;

        scheme0.verify(&message, &combined2.unwrap())?;
        scheme2.verify(&message, &combined3.unwrap())?;
        scheme3.verify(&message, &combined0.unwrap())?;

        Ok(())
    }
}
