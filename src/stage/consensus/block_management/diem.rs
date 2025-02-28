use super::{BlockManagement, CurBlockState};
use crate::config::DiemConfig;
use crate::context::{BlkCtx, TxnCtx};
use crate::peers::PeerMessenger;
use crate::protocol::block::{Block, BlockHeader};
use crate::protocol::crypto::threshold_signature::SignComb;
use crate::protocol::crypto::vector_snark::DummyMerkleTree;
use crate::protocol::crypto::{Hash, PubKey, Signature};
use crate::protocol::types::diem::{DiemBlock, LedgerCommitInfo, QuorumCert, TimeCert, VoteInfo};
use crate::stage::process_illusion;
use crate::transaction::{DiemTxn, Txn};
use crate::{CopycatError, NodeId};

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
    peer_messenger: Arc<PeerMessenger>,
    // states
    txn_pool: HashMap<Hash, (Arc<Txn>, Arc<TxnCtx>)>,
    block_pool: HashMap<Hash, (Arc<Block>, Arc<BlkCtx>)>,
    accounts: HashMap<u64, (PubKey, u64)>,
    state_merkle: DummyMerkleTree,
    // mempool
    pending_txns: VecDeque<Hash>,
    // consensus info
    current_round: u64,
    high_qc: QuorumCert,
    high_commit_qc: QuorumCert,
    last_tc: Option<Option<TimeCert>>, // outer option indicates if should start new round
    _notify: Notify,
}

impl DiemBlockManagement {
    pub fn new(
        id: NodeId,
        config: DiemConfig,
        delay: Arc<AtomicF64>,
        peer_messenger: Arc<PeerMessenger>,
    ) -> Self {
        let genesis_vote_info = VoteInfo {
            blk_id: Hash::zero(),
            round: 1,
            parent_id: Hash::zero(),
            parent_round: 0,
            exec_state_hash: Hash::zero(),
        };

        let genesis_commit_info = LedgerCommitInfo {
            commit_state_id: None,
            vote_info_hash: genesis_vote_info.compute_id().unwrap(),
        };

        let genesis_qc = QuorumCert {
            vote_info: genesis_vote_info,
            commit_info: genesis_commit_info,
            signatures: SignComb::new(),
            author: 0,
            author_signature: Signature::new(),
        };

        Self {
            id,
            blk_size: config.blk_size,
            delay,
            peer_messenger,
            txn_pool: HashMap::new(),
            block_pool: HashMap::new(),
            accounts: HashMap::new(),
            state_merkle: DummyMerkleTree::new(),
            pending_txns: VecDeque::new(),
            current_round: 2, // reserve round 0 and 1 for genesis
            high_qc: genesis_qc.clone(),
            high_commit_qc: genesis_qc,
            last_tc: None,
            _notify: Notify::new(),
        }
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

    async fn get_new_block(&mut self) -> Result<(Arc<Block>, Arc<BlkCtx>), CopycatError> {
        let mut txns = vec![];
        let mut txn_ctxs = vec![];
        let mut blk_size = 0usize;
        let mut speculative_exec_time = 0f64;
        let mut speculative_exec_distinct_writes = 0usize;
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
                    }
                },
            }

            txns.push(next_txn.clone());
            txn_ctxs.push(next_txn_ctx.clone());
            blk_size += txn_size;
        }

        // speculative execute
        let dur = self.state_merkle.append(speculative_exec_distinct_writes)?;
        process_illusion(dur + speculative_exec_time, &self.delay).await;

        // build block
        let diem_blk = DiemBlock {
            proposer: self.id,
            round: self.current_round,
            state_id: self.state_merkle.get_root(),
            qc: self.high_qc.clone(),
        };
        let diem_blk_id = diem_blk.compute_id(&txn_ctxs)?;
        let last_tc = std::mem::replace(&mut self.last_tc, None).unwrap();
        let signature = Signature::new(); // TODO: sign diem_blk_id
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
        block: Arc<Block>,
        ctx: Arc<BlkCtx>,
    ) -> Result<Vec<(Arc<Block>, Arc<BlkCtx>)>, CopycatError> {
        todo!()
    }

    async fn handle_pmaker_msg(&mut self, msg: Vec<u8>) -> Result<(), CopycatError> {
        todo!()
    }

    async fn handle_peer_blk_req(
        &mut self,
        peer: NodeId,
        msg: Vec<u8>,
    ) -> Result<(), CopycatError> {
        todo!()
    }

    fn report(&mut self) {}
}
