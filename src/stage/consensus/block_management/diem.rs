use super::{BlockManagement, CurBlockState};
use crate::config::DiemConfig;
use crate::context::{BlkCtx, TxnCtx};
use crate::peers::PeerMessenger;
use crate::protocol::block::{Block, BlockHeader};
use crate::protocol::crypto::signature::P2PSignature;
use crate::protocol::crypto::threshold_signature::ThresholdSignature;
use crate::protocol::crypto::vector_snark::DummyMerkleTree;
use crate::protocol::crypto::{Hash, PrivKey, PubKey};
use crate::protocol::types::diem::{
    DiemBlock, QuorumCert, TimeCert, GENESIS_QC, GENESIS_VOTE_INFO,
};
use crate::stage::pacemaker::diem::DiemPacemaker;
use crate::stage::process_illusion;
use crate::transaction::{DiemTxn, Txn};
use crate::{CopycatError, NodeId, SignatureScheme};

use atomic_float::AtomicF64;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::Notify;

use async_trait::async_trait;
use get_size::GetSize;

pub struct DiemBlockManagement {
    id: NodeId,
    blk_size: usize,
    delay: Arc<AtomicF64>,
    // states
    txn_pool: HashMap<Hash, (Arc<Txn>, Arc<TxnCtx>)>,
    accounts: HashMap<u64, (PubKey, u64)>,
    state_merkle: HashMap<Hash, DummyMerkleTree>,
    // mempool
    pending_txns: VecDeque<Hash>,
    // consensus info
    current_round: u64,
    high_qc: QuorumCert,
    high_commit_qc: QuorumCert,
    last_tc: Option<Option<TimeCert>>, // outer option indicates if should start new round
    // p2p communication
    all_nodes: Vec<NodeId>,
    _peer_messenger: Arc<PeerMessenger>,
    signature_scheme: SignatureScheme,
    peer_pks: HashMap<NodeId, PubKey>,
    sk: PrivKey,
    threshold_signature: Arc<dyn ThresholdSignature>,
    //
    _notify: Notify,
}

impl DiemBlockManagement {
    pub fn new(
        id: NodeId,
        p2p_signature: P2PSignature,
        threshold_signature: Arc<dyn ThresholdSignature>,
        config: DiemConfig,
        delay: Arc<AtomicF64>,
        peer_messenger: Arc<PeerMessenger>,
    ) -> Self {
        let (signature_scheme, peer_pks, sk) = p2p_signature;

        let mut all_nodes: Vec<NodeId> = peer_pks.keys().cloned().collect();
        all_nodes.sort();

        let genesis_qc = GENESIS_QC.clone();
        let current_round = 2;
        let last_tc = if DiemPacemaker::get_leader(current_round, &all_nodes) == id {
            Some(None)
        } else {
            None
        };

        Self {
            id,
            blk_size: config.blk_size,
            delay,
            _peer_messenger: peer_messenger,
            txn_pool: HashMap::new(),
            accounts: HashMap::new(),
            state_merkle: HashMap::new(),
            pending_txns: VecDeque::new(),
            current_round, // reserve round 0 and 1 for genesis
            high_qc: genesis_qc.clone(),
            high_commit_qc: genesis_qc,
            last_tc,
            all_nodes,
            signature_scheme,
            peer_pks,
            sk,
            threshold_signature,
            _notify: Notify::new(),
        }
    }

    async fn process_certificate_qc(&mut self, qc: &QuorumCert) -> Result<(), CopycatError> {
        let qc_round = qc.vote_info.round;

        if let Some(_id) = qc.commit_info.commit_state_id {
            // TODO: commit speculative execution results associated with id
            if qc_round > self.high_commit_qc.vote_info.round {
                self.high_commit_qc = qc.clone();
            }
        }

        if qc_round > self.high_qc.vote_info.round {
            self.high_qc = qc.clone();
        }

        if qc_round >= self.current_round {
            self.last_tc = None;
            self.current_round = qc_round + 1;
        }

        Ok(())
    }

    async fn process_certificate_tc(&mut self, tc: &Option<TimeCert>) -> Result<(), CopycatError> {
        if let Some(tc) = tc {
            let tc_round = tc.round;
            if tc_round >= self.current_round {
                self.last_tc = Some(Some(tc.clone()));
                self.current_round = tc_round + 1;
            }
        }

        Ok(())
    }

    fn validate_qc_signatures(&self, qc: &QuorumCert) -> Result<(bool, f64), CopycatError> {
        let mut verify_time = 0f64;

        let author_pk = match self.peer_pks.get(&qc.author) {
            Some(pk) => pk,
            None => return Ok((false, verify_time)),
        };

        let (check_result, dur) =
            self.signature_scheme
                .verify(author_pk, &qc.signatures, &qc.author_signature)?;
        verify_time += dur;
        if check_result == false {
            return Ok((false, verify_time));
        }

        let serialized_commit_info = bincode::serialize(&qc.commit_info)?;
        let (check_result, dur) = self
            .threshold_signature
            .verify(&serialized_commit_info, &qc.signatures)?;
        verify_time += dur;
        if check_result == false {
            return Ok((false, verify_time));
        }

        Ok((true, verify_time))
    }

    fn validate_tc_signatures(&self, tc: &Option<TimeCert>) -> Result<(bool, f64), CopycatError> {
        let tc = match tc {
            Some(tc) => tc,
            None => return Ok((true, 0f64)),
        };

        let mut verify_time = 0f64;

        for (node_id, high_qc_round) in &tc.tmo_high_qc_rounds {
            let author_pk = match self.peer_pks.get(node_id) {
                Some(pk) => pk,
                None => return Ok((false, verify_time)),
            };

            let data = (tc.round, *high_qc_round);
            let serialized = bincode::serialize(&data)?;
            let signature = match tc.tmo_signatures.get(node_id) {
                Some(sig) => sig,
                None => return Ok((false, verify_time)),
            };

            let (check_result, dur) =
                self.signature_scheme
                    .verify(author_pk, &serialized, &signature)?;
            verify_time += dur;
            if check_result == false {
                return Ok((false, verify_time));
            }
        }

        Ok((true, verify_time))
    }
}

#[async_trait]
impl BlockManagement for DiemBlockManagement {
    async fn record_new_txn(
        &mut self,
        txn: Arc<Txn>,
        ctx: Arc<TxnCtx>,
    ) -> Result<bool, CopycatError> {
        let txn_id = ctx.id;
        if self.txn_pool.contains_key(&txn_id) {
            return Ok(false);
        }

        self.txn_pool.insert(txn_id.clone(), (txn, ctx));
        self.pending_txns.push_back(txn_id);
        Ok(true)
    }

    // build the block only when we can propose a new block
    async fn prepare_new_block(&mut self) -> Result<CurBlockState, CopycatError> {
        Ok(CurBlockState::Full)
    }

    // when we receive a new QC from pmaker
    async fn wait_to_propose(&self) -> Result<(), CopycatError> {
        loop {
            if self.last_tc.is_some() {
                return Ok(());
            }
            self._notify.notified().await;
        }
    }

    // process_new_round_event()
    async fn get_new_block(&mut self) -> Result<(Arc<Block>, Arc<BlkCtx>), CopycatError> {
        let mut txns = vec![];
        let mut txn_ctxs = vec![];
        let mut blk_size = 0usize;
        let mut speculative_exec_time = 0f64;
        let mut speculative_exec_distinct_writes = 0usize;
        let mut speculative_exec_distinct_inserts = 0usize;

        // speculative execute
        let mut merkle_tree = if self.high_qc.vote_info.blk_id == GENESIS_VOTE_INFO.blk_id {
            DummyMerkleTree::new()
        } else {
            match self.state_merkle.get(&self.high_qc.vote_info.blk_id) {
                Some(merkle_tree) => merkle_tree.clone(),
                None => todo!(), // wait for dependency
            }
        };

        loop {
            let next_txn_id = match self.pending_txns.pop_front() {
                Some(txn) => txn,
                None => break, // no pending txns
            };

            let (next_txn, next_txn_ctx) = self.txn_pool.get(&next_txn_id).unwrap();
            // block is full
            let txn_size = next_txn.get_size();
            if blk_size + txn_size > self.blk_size {
                break;
            }

            let diem_txn = match next_txn.as_ref() {
                Txn::Diem { txn } => txn,
                _ => unreachable!(),
            };

            match diem_txn {
                DiemTxn::Txn {
                    sender,
                    seqno: _seqno, // TODO
                    payload,
                    max_gas_amount,
                    ..
                } => {
                    let (_, sender_balance) = match self.accounts.get(sender) {
                        Some(account) => account,
                        None => unreachable!(), // unknown sender - should not happen
                    };

                    if sender_balance < max_gas_amount {
                        continue; // invalid txn - not enough balance
                    }

                    speculative_exec_time += payload.script_runtime_sec;
                    if !payload.script_succeed {
                        continue; // invalid txn
                    }
                    speculative_exec_distinct_writes += payload.distinct_writes;
                }
                DiemTxn::Grant {
                    receiver,
                    receiver_key,
                    amount,
                } => match self.accounts.entry(*receiver) {
                    Entry::Occupied(mut e) => {
                        let (pubkey, balance) = e.get_mut();
                        if pubkey != receiver_key {
                            continue; // receivers do not match, ignore
                        }
                        *balance += amount; // TODO: move to speculative exec, do not change actual state yet
                    }
                    Entry::Vacant(e) => {
                        e.insert((receiver_key.clone(), *amount)); // TODO: move to speculative exec, do not change actual state yet
                        speculative_exec_distinct_inserts += 1; // TODO
                    }
                },
            }

            txns.push(next_txn.clone());
            txn_ctxs.push(next_txn_ctx.clone());
            blk_size += txn_size;
        }

        let append_dur = merkle_tree.append(speculative_exec_distinct_inserts)?;
        let update_dur = merkle_tree.update(speculative_exec_distinct_writes)?;
        process_illusion(append_dur + update_dur + speculative_exec_time, &self.delay).await;

        // build block
        let diem_blk = DiemBlock {
            proposer: self.id,
            round: self.current_round,
            state_id: merkle_tree.get_root(),
            qc: self.high_qc.clone(),
        };
        let diem_blk_id = diem_blk.compute_id(&txn_ctxs)?;
        self.state_merkle.insert(diem_blk_id, merkle_tree);

        let serialized = bincode::serialize(&diem_blk_id)?;
        let (signature, dur) = self.signature_scheme.sign(&self.sk, &serialized)?;
        let last_tc = std::mem::replace(&mut self.last_tc, None).unwrap();
        process_illusion(dur, &self.delay).await;
        let blk_header = BlockHeader::Diem {
            block: diem_blk,
            high_commit_qc: self.high_commit_qc.clone(),
            last_round_tc: last_tc,
            signature,
        };
        let blk = Arc::new(Block {
            header: blk_header,
            txns,
        });
        let blk_ctx = Arc::new(BlkCtx {
            id: diem_blk_id,
            txn_ctx: txn_ctxs,
        });

        Ok((blk, blk_ctx))
    }

    async fn validate_block(
        &mut self,
        src: NodeId,
        block: Arc<Block>,
        ctx: Arc<BlkCtx>,
    ) -> Result<Vec<(Arc<Block>, Arc<BlkCtx>)>, CopycatError> {
        let (diem_blk, high_commit_qc, last_round_tc, signature) = match &block.header {
            BlockHeader::Diem {
                block,
                high_commit_qc,
                last_round_tc,
                signature,
            } => (block, high_commit_qc, last_round_tc, signature),
            _ => unreachable!(),
        };

        // validate signatures
        let qc_valid = if diem_blk.qc == *GENESIS_QC {
            true
        } else {
            let (valid, verify_time) = self.validate_qc_signatures(&diem_blk.qc)?;
            process_illusion(verify_time, &self.delay).await;
            if valid {
                self.process_certificate_qc(&diem_blk.qc).await?;
            }
            valid
        };

        let high_commit_qc_valid = if *high_commit_qc == *GENESIS_QC {
            true
        } else {
            let (valid, verify_time) = self.validate_qc_signatures(high_commit_qc)?;
            process_illusion(verify_time, &self.delay).await;
            if valid {
                self.process_certificate_qc(high_commit_qc).await?;
            }
            valid
        };

        let tc_valid = {
            let (valid, verify_time) = self.validate_tc_signatures(last_round_tc)?;
            process_illusion(verify_time, &self.delay).await;
            if valid {
                self.process_certificate_tc(last_round_tc).await?;
            }
            valid
        };

        let signature_valid = match self.peer_pks.get(&diem_blk.proposer) {
            Some(pk) => {
                let serialized = bincode::serialize(&ctx.id)?;
                let (valid, verify_time) =
                    self.signature_scheme.verify(&pk, &serialized, signature)?;
                process_illusion(verify_time, &self.delay).await;
                valid
            }
            None => false,
        };

        if !(qc_valid && high_commit_qc_valid && tc_valid && signature_valid) {
            return Ok(vec![]);
        }

        let leader = DiemPacemaker::get_leader(self.current_round, &self.all_nodes);
        if diem_blk.round != self.current_round || src != leader || diem_blk.proposer != leader {
            return Ok(vec![]);
        }

        // TODO: speculative exec
        let mut speculative_exec_time = 0f64;
        let mut speculative_exec_distinct_writes = 0usize;
        let mut speculative_exec_distinct_inserts = 0usize;

        let mut merkle_tree = if diem_blk.qc.vote_info.blk_id == GENESIS_VOTE_INFO.blk_id {
            DummyMerkleTree::new()
        } else {
            match self.state_merkle.get(&diem_blk.qc.vote_info.blk_id) {
                Some(merkle_tree) => merkle_tree.clone(),
                None => todo!(), // TODO: missing depending blocks
            }
        };

        for txn in &block.txns {
            let diem_txn = match txn.as_ref() {
                Txn::Diem { txn } => txn,
                _ => unreachable!(),
            };

            match diem_txn {
                DiemTxn::Txn {
                    sender,
                    seqno: _seqno, // TODO
                    payload,
                    max_gas_amount,
                    ..
                } => {
                    let (_, sender_balance) = match self.accounts.get(sender) {
                        Some(account) => account,
                        None => return Ok(vec![]), // invalid txn - unknown sender
                    };

                    if sender_balance < max_gas_amount {
                        return Ok(vec![]); // invalid txn - not enough balance
                    }

                    speculative_exec_time += payload.script_runtime_sec;
                    if !payload.script_succeed {
                        return Ok(vec![]); // invalid txn
                    }
                    speculative_exec_distinct_writes += payload.distinct_writes;
                }
                DiemTxn::Grant {
                    receiver,
                    receiver_key,
                    amount,
                } => match self.accounts.entry(*receiver) {
                    Entry::Occupied(mut e) => {
                        let (pubkey, balance) = e.get_mut();
                        if pubkey != receiver_key {
                            continue; // receivers do not match, ignore
                        }
                        *balance += amount; // TODO: move to speculative exec, do not change actual state yet
                    }
                    Entry::Vacant(e) => {
                        e.insert((receiver_key.clone(), *amount)); // TODO: move to speculative exec, do not change actual state yet
                        speculative_exec_distinct_inserts += 1; // TODO
                    }
                },
            }
        }

        let append_dur = merkle_tree.append(speculative_exec_distinct_inserts)?;
        let update_dur = merkle_tree.update(speculative_exec_distinct_writes)?;
        process_illusion(append_dur + update_dur + speculative_exec_time, &self.delay).await;
        if !merkle_tree.verify_root(&diem_blk.state_id)? {
            return Ok(vec![]); // speculative exec results do not match
        }
        self.state_merkle.insert(ctx.id, merkle_tree);

        return Ok(vec![(block, ctx)]);
    }

    async fn handle_pmaker_msg(&mut self, msg: Vec<u8>) -> Result<(), CopycatError> {
        let qc: QuorumCert = bincode::deserialize(&msg)?;
        self.process_certificate_qc(&qc).await?;
        self.last_tc = Some(None);
        Ok(())
    }

    async fn handle_peer_blk_req(
        &mut self,
        _peer: NodeId,
        _msg: Vec<u8>,
    ) -> Result<(), CopycatError> {
        todo!()
    }

    fn report(&mut self) {}
}
