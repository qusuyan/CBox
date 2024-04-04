use super::Pacemaker;
use crate::utils::CopycatError;

use async_trait::async_trait;

use std::sync::Arc;

pub struct DummyPacemaker {}

#[async_trait]
impl Pacemaker for DummyPacemaker {
    async fn wait_to_propose(&self) -> Result<Arc<Vec<u8>>, CopycatError> {
        let notify = tokio::sync::Notify::new();
        loop {
            notify.notified().await;
        }
    }
}
