use std::fmt::Debug;

use copycat_utils::{CopycatError, NodeId};
use tokio::{sync::mpsc, task::JoinHandle};
use mailbox_client::ClientStub;

use crate::composition::SystemCompose;
use crate::stage::txn_validation::{get_txn_validation, TxnValidationType};

pub struct Node<Txn> {
    req_send: mpsc::Sender<Txn>,
    _txn_validation_handle: JoinHandle<()>,
}

impl<Txn> Node<Txn>
where
    Txn: 'static + Debug + Sync + Send,
{
    pub async fn init(id: NodeId, compose: SystemCompose) -> Result<Self, CopycatError> {
        let transport: ClientStub<Vec<u8>> = ClientStub::new(id)?;

        let (req_send, req_recv) = mpsc::channel(1024 * 1024);
        let (validated_txn_send, validated_txn_recv) = mpsc::channel(1024 * 1024);

        let _txn_validation_handle = tokio::spawn(Self::txn_validation_thread(
            id,
            compose.txn_validation_stage,
            req_recv,
            validated_txn_send,
        ));

        Ok(Self {
            req_send,
            _txn_validation_handle,
        })
    }
}

impl<Txn> Node<Txn>
where
    Txn: 'static + Debug + Sync + Send,
{
    async fn txn_validation_thread(
        id: NodeId,
        txn_validation_type: TxnValidationType,
        mut req_recv: mpsc::Receiver<Txn>,
        validated_txn_send: mpsc::Sender<Txn>,

    ) {
        let txn_validation_stage = get_txn_validation::<Txn>(id, txn_validation_type);

        loop {
            let txn = match req_recv.recv().await {
                Some(txn) => txn,
                None => {
                    log::error!("Node {id}: request channel closed");
                    continue;
                }
            };

            if txn_validation_stage.validate(&txn) {
                if let Err(e) = validated_txn_send.send(txn).await {
                    log::error!("Node {id}: failed to send from txn_validation to txn_dissemination: {e:?}");
                }
            } else {
                log::warn!("Node {id}: got invalid txn, ignoring...");
            }
        }
    }
}
