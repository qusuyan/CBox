pub mod commit;
pub mod consensus;
pub mod pacemaker;
pub mod txn_dissemination;
pub mod txn_validation;

use atomic_float::AtomicF64;

use std::sync::atomic::Ordering;
use tokio::time::Duration;

async fn pass() {}

struct DelayPool {
    delay: AtomicF64,
}

impl DelayPool {
    fn new() -> Self {
        Self {
            delay: AtomicF64::new(0f64),
        }
    }

    async fn process_illusion(&self, timeout: f64) {
        // tokio sleep granularity is 1 ms
        let sleep_time = self.delay.fetch_add(timeout, Ordering::Relaxed);
        if sleep_time + timeout > 0.005 {
            tokio::time::sleep(Duration::from_secs_f64(sleep_time + timeout)).await;
            self.delay.store(0f64, Ordering::Relaxed);
        }
        // if timeout < 0.005 {
        //     let rand = {
        //         let mut rng = thread_rng();
        //         rng.gen::<f64>()
        //     };
        //     if rand < timeout / 0.005 {
        //         tokio::time::sleep(Duration::from_millis(5)).await
        //     }
        // } else {
        //     tokio::time::sleep(Duration::from_secs_f64(timeout)).await
        // }
    }

    #[allow(dead_code)]
    fn get_current_delay(&self) -> f64 {
        self.delay.load(Ordering::Relaxed)
    }
}
