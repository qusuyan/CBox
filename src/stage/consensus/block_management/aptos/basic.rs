use super::{BlockManagement, CurBlockState};
use crate::config::AptosDiemConfig;
use crate::context::{BlkCtx, TxnCtx};
use crate::peers::PeerMessenger;
use crate::protocol::block::Block;
use crate::protocol::crypto::signature::P2PSignature;
use crate::protocol::crypto::{PrivKey, PubKey};
use crate::stage::DelayPool;
use crate::transaction::Txn;
use crate::{CopycatError, NodeId, SignatureScheme};

use async_trait::async_trait;

use std::collections::HashMap;
use std::sync::Arc;

pub struct AptosBlockManagement {
    id: NodeId,
    blk_size: usize,
    // p2p communication
    peer_messenger: Arc<PeerMessenger>,
    signature_scheme: SignatureScheme,
    peer_pks: HashMap<NodeId, PubKey>,
    sk: PrivKey,
    //
    delay: Arc<DelayPool>,
}

impl AptosBlockManagement {
    pub fn new(
        id: NodeId,
        p2p_signature: P2PSignature,
        config: AptosDiemConfig,
        delay: Arc<DelayPool>,
        peer_messenger: Arc<PeerMessenger>,
    ) -> Self {
        let (signature_scheme, peer_pks, sk) = p2p_signature;

        Self {
            id,
            blk_size: config.narwhal_blk_size,
            peer_messenger,
            signature_scheme,
            peer_pks,
            sk,
            delay,
        }
    }
}

#[async_trait]
impl BlockManagement for AptosBlockManagement {
    async fn record_new_txn(
        &mut self,
        txn: Arc<Txn>,
        ctx: Arc<TxnCtx>,
    ) -> Result<bool, CopycatError> {
        todo!()
    }

    async fn prepare_new_block(&mut self) -> Result<CurBlockState, CopycatError> {
        todo!()
    }

    async fn wait_to_propose(&self) -> Result<(), CopycatError> {
        todo!()
    }

    async fn get_new_block(&mut self) -> Result<(Arc<Block>, Arc<BlkCtx>), CopycatError> {
        todo!()
    }

    async fn validate_block(
        &mut self,
        src: NodeId,
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
