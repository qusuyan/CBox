use super::{SignComb, SignPart, ThresholdSignature};
use crate::{CopycatError, NodeId};

use rand::prelude::*;
use rand_chacha::ChaCha8Rng;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use frost_secp256k1 as frost;

///! TODO: FROST DOES NOT ALWAYS WORK!
/// FROST requires signer to know before signing who are the participants for the current round
/// this is not suitable for distributed systems where participants can go down any time

pub struct FrostThresholdSignature {
    me: NodeId,
    node_id_map: BTreeMap<NodeId, frost::Identifier>,
    id_node_map: BTreeMap<frost::Identifier, NodeId>,
    signing_key: frost::keys::KeyPackage,
    verifying_key: frost::keys::PublicKeyPackage,
    // TODO: each message should have different nonce and commitment but require extra RTT
    nonce: frost::round1::SigningNonces,
    commitments: BTreeMap<frost::Identifier, frost::round1::SigningCommitments>,
}

impl FrostThresholdSignature {
    pub fn setup(
        nodes: &HashSet<NodeId>,
        k: u16,
        seed: u64,
    ) -> Result<HashMap<NodeId, Arc<dyn ThresholdSignature>>, CopycatError> {
        let n = nodes.len().try_into()?;
        let mut rng = ChaCha8Rng::seed_from_u64(seed);

        let node_id_map = nodes
            .iter()
            .map(|node| frost::Identifier::derive(&node.to_le_bytes()).map(|id| (*node, id)))
            .collect::<Result<BTreeMap<NodeId, frost::Identifier>, frost::Error>>()?;
        let ids: Vec<frost::Identifier> = node_id_map.values().cloned().collect();
        let id_node_map = node_id_map
            .iter()
            .map(|(node, id)| (*id, *node))
            .collect::<BTreeMap<_, _>>();

        let (signing_shares, verifying_key) = frost::keys::generate_with_dealer(
            n,
            k,
            frost::keys::IdentifierList::Custom(&ids),
            &mut rng,
        )?;

        let mut signing_keys = BTreeMap::new();
        let mut commitments = BTreeMap::new();
        for (id, secret_share) in signing_shares {
            let key_package = frost::keys::KeyPackage::try_from(secret_share)?;
            let (nonce, commitment) = frost::round1::commit(key_package.signing_share(), &mut rng);
            signing_keys.insert(id, (key_package, nonce));
            commitments.insert(id, commitment);
        }

        let mut quorum: HashMap<u64, Arc<dyn ThresholdSignature>> = HashMap::new();
        for (node, id) in node_id_map.iter() {
            let (signing_key, nonce) = signing_keys.remove(id).unwrap();
            let scheme = Self {
                me: *node,
                node_id_map: node_id_map.clone(),
                id_node_map: id_node_map.clone(),
                signing_key,
                verifying_key: verifying_key.clone(),
                nonce,
                commitments: commitments.clone(),
            };
            quorum.insert(*node, Arc::new(scheme));
        }

        Ok(quorum)
    }
}

impl ThresholdSignature for FrostThresholdSignature {
    fn sign(&self, input: &[u8]) -> Result<(SignPart, f64), CopycatError> {
        let signing_package = frost::SigningPackage::new(self.commitments.clone(), input);
        let signature_share =
            frost::round2::sign(&signing_package, &self.nonce, &self.signing_key)?;
        Ok((signature_share.serialize(), 0f64))
    }

    fn aggregate(
        &self,
        input: &[u8],
        parts: &mut HashMap<NodeId, SignPart>,
    ) -> Result<(Option<SignComb>, f64), CopycatError> {
        // let signing_package = frost::SigningPackage::new(self.commitments.clone(), input);
        let mut signature_shares = BTreeMap::new();
        let mut invalid_list = vec![];
        for (node, serialized_part) in parts.iter() {
            let identifier = match self.node_id_map.get(node) {
                Some(id) => id,
                None => {
                    pf_warn!(self.me; "Drop invalid Node ID {}", node);
                    invalid_list.push(*node);
                    continue;
                }
            };

            let signature_share = match frost::round2::SignatureShare::deserialize(&serialized_part)
            {
                Ok(part) => part,
                Err(_) => {
                    pf_warn!(self.me; "Drop invalid signature share from Node {}", node);
                    invalid_list.push(*node);
                    continue;
                }
            };

            signature_shares.insert(*identifier, signature_share);
        }

        for node in invalid_list {
            parts.remove(&node);
        }

        let mut commitments = BTreeMap::new();
        for id in signature_shares.keys() {
            let commitment = self.commitments.get(id).unwrap();
            commitments.insert(*id, *commitment);
        }
        let signing_package = frost::SigningPackage::new(commitments, &input);

        print!("{:?}", signature_shares);

        match frost::aggregate(&signing_package, &signature_shares, &self.verifying_key) {
            Ok(signature) => Ok((Some(signature.serialize()?), 0f64)),
            Err(e) => match e {
                frost::Error::InvalidSecretShare {
                    culprit: Some(culprit),
                } => {
                    match self.id_node_map.get(&culprit) {
                        Some(node) => {
                            pf_warn!(self.me; "Drop invalid signature share from Node {}", node);
                            parts.remove(node);
                        }
                        None => pf_error!(self.me; "Unexpected identifier found"),
                    };
                    Ok((None, 0f64))
                }
                frost::Error::InvalidSignatureShare { culprit } => {
                    println!("Invalid signature share {:?}", culprit);
                    match self.id_node_map.get(&culprit) {
                        Some(node) => {
                            parts.remove(node);
                        }
                        None => pf_error!(self.me; "Unexpected identifier found"),
                    };
                    Ok((None, 0f64))
                }
                frost::Error::InvalidProofOfKnowledge { culprit } => {
                    match self.id_node_map.get(&culprit) {
                        Some(node) => {
                            pf_warn!(self.me; "Drop invalid signature share from Node {}", node);
                            parts.remove(node);
                        }
                        None => pf_error!(self.me; "Unexpected identifier found"),
                    };
                    Ok((None, 0f64))
                }
                _ => Err(e.into()),
            },
        }
    }

    fn verify(&self, input: &[u8], signature: &SignComb) -> Result<(bool, f64), CopycatError> {
        let deserialized_signature = frost::Signature::deserialize(signature)?;
        let valid = match self
            .verifying_key
            .verifying_key()
            .verify(&input, &deserialized_signature)
        {
            Ok(()) => true,
            Err(e) => {
                pf_warn!(self.me; "Invalid signature: {:?}", e);
                false
            }
        };
        Ok((valid, 0f64))
    }
}

#[cfg(test)]
mod frost_threshold_signature_test {

    use super::FrostThresholdSignature;
    use crate::CopycatError;

    use bincode;
    use std::collections::{HashMap, HashSet};

    #[test]
    fn test_frost_threshold_signature() -> Result<(), CopycatError> {
        let message = bincode::serialize("test message content")?;

        let nodes = HashSet::from_iter(0..5);
        let mut schemes = FrostThresholdSignature::setup(&nodes, 3, 241241)?;
        let scheme0 = schemes.remove(&0).unwrap();
        let scheme2 = schemes.remove(&2).unwrap();
        let scheme3 = schemes.remove(&3).unwrap();

        let part0 = scheme0.sign(&message)?;
        let part2 = scheme2.sign(&message)?;
        let part3 = scheme3.sign(&message)?;

        let mut parts = HashMap::new();
        parts.insert(0, part0);
        parts.insert(2, part2);
        parts.insert(3, part3);

        // let combined0 = scheme0.aggregate(&message, &mut parts)?;
        // let combined2 = scheme2.aggregate(&message, &mut parts)?;
        // let combined3 = scheme3.aggregate(&message, &mut parts)?;

        // scheme0.verify(&message, &combined2)?;
        // scheme2.verify(&message, &combined3)?;
        // scheme3.verify(&message, &combined0)?;

        Ok(())
    }
}
