use super::Pacemaker;
use crate::config::AptosDiemConfig;
use crate::protocol::crypto::signature::P2PSignature;
use crate::protocol::types::aptos::CoA;
use crate::{CopycatError, NodeId};

use async_trait::async_trait;
use tokio::sync::Notify;

use ::std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet, VecDeque};

pub struct AptosPmaker {
    _id: NodeId,
    num_faulty: usize,
    all_nodes: Vec<NodeId>,
    certificates: HashMap<u64, Vec<CoA>>,
    pending: VecDeque<u64>,
    sent: HashSet<u64>,
    _notify: Notify,
}

impl AptosPmaker {
    pub fn new(id: NodeId, _config: AptosDiemConfig, p2p_signature: P2PSignature) -> Self {
        let (_, peer_pks, _) = p2p_signature;

        let mut all_nodes: Vec<NodeId> = peer_pks.keys().cloned().collect();
        all_nodes.sort();
        let num_faulty = (all_nodes.len() - 1) / 3;

        Self {
            _id: id,
            all_nodes,
            num_faulty,
            certificates: HashMap::new(),
            pending: VecDeque::new(),
            sent: HashSet::new(),
            _notify: Notify::new(),
        }
    }
}

#[async_trait]
impl Pacemaker for AptosPmaker {
    async fn wait_to_propose(&self) -> Result<(), CopycatError> {
        if self.pending.is_empty() {
            // wait forever
            self._notify.notified().await;
        }
        Ok(())
    }

    async fn get_propose_msg(&mut self) -> Result<Vec<u8>, CopycatError> {
        let pending_round = self.pending.pop_front().unwrap();
        let coa_list = self.certificates.remove(&pending_round).unwrap();
        Ok((bincode::serialize(&coa_list))?)
    }

    async fn handle_feedback(&mut self, feedback: Vec<u8>) -> Result<(), CopycatError> {
        let coa: CoA = bincode::deserialize(&feedback)?;
        let round = coa.round;
        let certificate_list = match self.certificates.entry(round) {
            Entry::Occupied(e) => e.into_mut(),
            Entry::Vacant(e) => {
                if self.sent.contains(&round) {
                    // already sent to block management, do nothing
                    return Ok(());
                } else {
                    e.insert(vec![])
                }
            }
        };
        certificate_list.push(coa);

        if self.sent.contains(&round) {
            // already in the pending queue, do nothing
            return Ok(());
        }

        if certificate_list.len() >= self.all_nodes.len() - self.num_faulty {
            self.sent.insert(round);
            self.pending.push_back(round);
        }

        Ok(())
    }

    async fn handle_peer_msg(&mut self, _src: NodeId, _msg: Vec<u8>) -> Result<(), CopycatError> {
        unreachable!()
    }
}
