mod error;
use std::time::Duration;

pub use error::CopycatError;

#[macro_use]
pub mod log;

pub mod parse_config;

mod parse_topo;
pub use parse_topo::{fully_connected_topology, get_neighbors, get_topology};

mod report;
pub use report::{get_report_timer, get_timer_interval, start_report_timer};

pub mod time_queue;

use crate::consts::SPIN_WAIT_CUTOFF;
use tokio::time::Instant;

pub type NodeId = u64;

pub async fn sleep_until(time: Instant) {
    let wakeup_time = time - Duration::from_millis(1);
    loop {
        let sleep_amount = wakeup_time.duration_since(Instant::now());
        if sleep_amount > *SPIN_WAIT_CUTOFF {
            tokio::time::sleep(sleep_amount).await;
        } else if sleep_amount > Duration::from_secs(0) {
            tokio::task::yield_now().await;
        } else {
            return;
        }
    }
}
