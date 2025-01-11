use std::time::Duration;

use atomic_float::AtomicF64;

pub mod commit;
pub mod consensus;
pub mod pacemaker;
pub mod txn_dissemination;
pub mod txn_validation;

use std::sync::atomic::Ordering;

async fn pass() {}

async fn process_illusion(timeout: Duration, delay: &AtomicF64) {
    if timeout.as_millis() < 1 {
        // tokio sleep granularity is 1 ms
        delay.fetch_add(timeout.as_secs_f64(), Ordering::Relaxed);
    } else {
        tokio::time::sleep(timeout).await;
    }
}
