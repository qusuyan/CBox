mod dummy;
use dummy::DummyBlockCreation;

use crate::state::ChainState;
use copycat_protocol::block::Block;
use copycat_protocol::transaction::Txn;
use copycat_protocol::ChainType;
use copycat_utils::{CopycatError, NodeId};

use async_trait::async_trait;

use std::sync::Arc;
use tokio::sync::mpsc;

#[async_trait]
pub trait BlockCreation: Sync + Send {
    async fn new_txn(&mut self, txn: Arc<Txn>) -> Result<(), CopycatError>;
    async fn new_block(&mut self) -> Result<Arc<Block>, CopycatError>;
}

fn get_blk_creation(
    chain_type: ChainType,
    state: Arc<ChainState>,
    pacemaker_recv: mpsc::Receiver<Arc<Vec<u8>>>,
) -> Box<dyn BlockCreation> {
    match chain_type {
        ChainType::Dummy => Box::new(DummyBlockCreation::new()),
        ChainType::Bitcoin => todo!(),
    }
}

pub async fn block_creation_thread(
    id: NodeId,
    chain_type: ChainType,
    state: Arc<ChainState>,
    mut txn_ready_recv: mpsc::Receiver<Arc<Txn>>,
    pacemaker_recv: mpsc::Receiver<Arc<Vec<u8>>>,
    new_block_send: mpsc::Sender<Arc<Block>>,
) {
    log::trace!("Node {id}: Txn Validation stage starting...");

    let mut block_creation_stage = get_blk_creation(chain_type, state, pacemaker_recv);

    loop {
        tokio::select! {
            new_txn = txn_ready_recv.recv() => {
                match new_txn {
                    Some(txn) => {
                        log::trace!("Node {id}: got new txn {txn:?}");
                        if let Err(e) = block_creation_stage.new_txn(txn).await {
                            log::error!("Node {id}: failed to record new txn: {e:?}");
                            continue;
                        }
                    },
                    None => {
                        log::error!("Node {id}: txn_ready pipe closed unexpectedly");
                        continue;
                    }
                }
            },
            new_blk = block_creation_stage.new_block() => {
                match new_blk {
                    Ok(block) => {
                        log::trace!("Node {id}: proposing new block {block:?}");
                        if let Err(e) = new_block_send.send(block).await {
                            log::error!("Node {id}: failed to send to new_block pipe: {e:?}");
                            continue;
                        }
                    },
                    Err(e) => {
                        log::error!("Node {id}: error creating new block: {e:?}");
                        continue;
                    }
                }
            },
        }
    }
}
