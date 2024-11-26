// atomic based implementation:
use crate::CopycatError;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use tokio::sync::{OwnedSemaphorePermit, Semaphore, SemaphorePermit};
use tokio::time::Instant;

use atomic_float::AtomicF64;

pub struct VCore<'a> {
    _ticket: SemaphorePermit<'a>,
    core_group: &'a VCoreGroup,
}

impl Drop for VCore<'_> {
    fn drop(&mut self) {
        self.core_group.release();
        // drop(self.ticket) called implicitly
    }
}

pub struct VCoreOwned {
    _ticket: OwnedSemaphorePermit,
    core_group: Arc<VCoreGroup>,
}

impl Drop for VCoreOwned {
    fn drop(&mut self) {
        self.core_group.release();
    }
}

pub struct VCoreGroup {
    cores: Arc<Semaphore>,
    max_cores: usize,
    start_time: Instant,
    last_wakeup: AtomicU64,
    utilization: AtomicF64,
}

impl VCoreGroup {
    pub fn new(max_cores: usize) -> Self {
        let now = Instant::now();
        let last_wakeup = 0; // since num tickets is 0 and time since start is also 0

        Self {
            cores: Arc::new(Semaphore::new(max_cores)),
            max_cores,
            start_time: now,
            last_wakeup: AtomicU64::new(last_wakeup),
            utilization: AtomicF64::new(0f64),
        }
    }

    pub fn get_total_cores(&self) -> usize {
        self.max_cores
    }

    pub async fn acquire(&self) -> Result<VCore<'_>, CopycatError> {
        let ticket = self.cores.acquire().await?;

        let mut past = self.last_wakeup.load(Ordering::SeqCst);
        let (now, past, tickets) = loop {
            let now = Instant::now().duration_since(self.start_time).as_secs_f32();
            let now_bits = now.to_bits() as u64;
            let tickets = (past & 0xFFFFFFFF) + 1;
            let curr = (now_bits << 32) | tickets;
            match self
                .last_wakeup
                .compare_exchange(past, curr, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => break (now, past, tickets - 1),
                Err(val) => past = val,
            }
        };

        let last_ts = f32::from_bits((past >> 32) as u32);
        let time_window = now - last_ts;
        let window_util = time_window as f64 * tickets as f64;
        self.utilization.fetch_add(window_util, Ordering::Release);

        Ok(VCore {
            _ticket: ticket,
            core_group: self,
        })
    }

    pub async fn acquire_owned(self: Arc<Self>) -> Result<VCoreOwned, CopycatError> {
        let ticket = self.cores.clone().acquire_owned().await?;

        let mut past = self.last_wakeup.load(Ordering::SeqCst);
        let (now, past, tickets) = loop {
            let now = Instant::now().duration_since(self.start_time).as_secs_f32();
            let now_bits = now.to_bits() as u64;
            let tickets = (past & 0xFFFFFFFF) + 1;
            let curr = (now_bits << 32) | tickets;
            match self
                .last_wakeup
                .compare_exchange(past, curr, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => break (now, past, tickets - 1),
                Err(val) => past = val,
            }
        };

        let last_ts = f32::from_bits((past >> 32) as u32);
        let time_window = now - last_ts;
        let window_util = time_window as f64 * tickets as f64;
        self.utilization.fetch_add(window_util, Ordering::Release);

        Ok(VCoreOwned {
            _ticket: ticket,
            core_group: self,
        })
    }

    fn release(&self) {
        let mut past = self.last_wakeup.load(Ordering::SeqCst);
        let (now, past, tickets) = loop {
            let now = Instant::now().duration_since(self.start_time).as_secs_f32();
            let now_bits = now.to_bits() as u64;
            let tickets = (past & 0xFFFFFFFF) - 1;
            let curr = (now_bits << 32) | tickets;
            match self
                .last_wakeup
                .compare_exchange(past, curr, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => break (now, past, tickets + 1),
                Err(val) => past = val,
            }
        };

        let last_ts = f32::from_bits((past >> 32) as u32);
        let time_window = now - last_ts;
        let window_util = time_window as f64 * tickets as f64;
        self.utilization.fetch_add(window_util, Ordering::Release);
    }

    // TODO: this read is sloppy
    pub fn get_utilization(&self) -> f64 {
        let ts = self.last_wakeup.load(Ordering::SeqCst);
        let util = self.utilization.load(Ordering::Acquire);
        let now = Instant::now().duration_since(self.start_time).as_secs_f32();

        let time = f32::from_bits((ts >> 32) as u32);
        let tickets = ts & 0xFFFFFFFF;

        let unaccounted = tickets as f64 * (now - time) as f64;
        (util + unaccounted) / now as f64
    }

    pub fn get_unused(&self) -> f64 {
        self.max_cores as f64 - self.get_utilization()
    }
}

#[cfg(test)]
mod vcores_test {
    use super::VCoreGroup;
    use std::sync::Arc;
    use tokio::time::Duration;

    #[tokio::test]
    async fn test_acquire_release() {
        let core_group = VCoreGroup::new(2);

        let ticket = core_group.acquire().await;
        tokio::time::sleep(Duration::from_millis(10)).await;
        let util = core_group.get_utilization();
        print!("{}\n", util);
        assert!((util - 1f64).abs() < 0.05);

        drop(ticket);
        tokio::time::sleep(Duration::from_millis(10)).await;
        let util = core_group.get_utilization();
        print!("{}\n", util);
        assert!((util - 0.5).abs() < 0.05);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_concurrent() {
        let core_group = Arc::new(VCoreGroup::new(2));

        let cores1 = core_group.clone();
        let task1 = tokio::spawn(async move {
            let _ticket = cores1.acquire().await;
            tokio::time::sleep(Duration::from_millis(10)).await;
        });

        let cores2 = core_group.clone();
        let task2 = tokio::spawn(async move {
            let _ticket = cores2.acquire().await;
            tokio::time::sleep(Duration::from_millis(5)).await;
        });

        task1.await.unwrap();
        task2.await.unwrap();

        let util = core_group.get_utilization();
        print!("{}\n", util);
        assert!((util - 1.5).abs() < 0.05);

        tokio::time::sleep(Duration::from_millis(5)).await;
        let util = core_group.get_utilization();
        print!("{}\n", util);
        assert!((util - 1f64).abs() < 0.05);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_waiting_for_ticket() {
        let core_group = Arc::new(VCoreGroup::new(2));

        let cores1 = core_group.clone();
        let task1 = tokio::spawn(async move {
            let _ticket = cores1.acquire().await;
            tokio::time::sleep(Duration::from_millis(10)).await;
        });

        let cores2 = core_group.clone();
        let task2 = tokio::spawn(async move {
            let _ticket = cores2.acquire().await;
            tokio::time::sleep(Duration::from_millis(5)).await;
        });

        let cores3 = core_group.clone();
        let task3 = tokio::spawn(async move {
            let _ticket = cores3.acquire().await;
            tokio::time::sleep(Duration::from_millis(5)).await;
        });

        task1.await.unwrap();
        task2.await.unwrap();
        task3.await.unwrap();

        let util = core_group.get_utilization();
        print!("{}\n", util);
        assert!((util - 2f64).abs() < 0.3);

        tokio::time::sleep(Duration::from_millis(10)).await;
        let util = core_group.get_utilization();
        print!("{}\n", util);
        assert!((util - 1f64).abs() < 0.05);
    }
}

// lock-based implementation:

// use crate::CopycatError;

// use tokio::sync::{Semaphore, SemaphorePermit};
// use tokio::time::Instant;

// use parking_lot;

// pub struct VCore<'a> {
//     _ticket: SemaphorePermit<'a>,
//     core_group: &'a VCoreGroup,
// }

// impl Drop for VCore<'_> {
//     fn drop(&mut self) {
//         self.core_group.release();
//         // drop(self.ticket) called implicitly
//     }
// }

// #[derive(Clone)]
// struct VCoreGroupStats {
//     last_wakeup: Instant,
//     tickets: usize,
//     utilization: f64,
// }

// pub struct VCoreGroup {
//     cores: Semaphore,
//     start_time: Instant,
//     stats: parking_lot::Mutex<VCoreGroupStats>,
// }

// impl VCoreGroup {
//     pub fn new(max_cores: usize) -> Self {
//         let now = Instant::now();

//         Self {
//             cores: Semaphore::new(max_cores),
//             start_time: now,
//             stats: parking_lot::Mutex::new(VCoreGroupStats {
//                 last_wakeup: now,
//                 tickets: 0,
//                 utilization: 0f64,
//             }),
//         }
//     }

//     pub async fn acquire(&self) -> Result<VCore<'_>, CopycatError> {
//         let ticket = self.cores.acquire().await?;

//         {
//             let mut guard = self.stats.lock();
//             let now = Instant::now();
//             let time_elapsed = now.duration_since(guard.last_wakeup).as_secs_f64();
//             guard.utilization += time_elapsed * guard.tickets as f64;
//             guard.tickets += 1;
//             guard.last_wakeup = now;
//         }

//         Ok(VCore {
//             _ticket: ticket,
//             core_group: self,
//         })
//     }

//     fn release(&self) {
//         let mut guard = self.stats.lock();
//         let now = Instant::now();
//         let time_elapsed = now.duration_since(guard.last_wakeup).as_secs_f64();
//         guard.utilization += time_elapsed * guard.tickets as f64;
//         guard.tickets -= 1;
//         guard.last_wakeup = now;
//     }

//     pub fn get_utilization(&self) -> f64 {
//         let (stats, now) = {
//             let guard = self.stats.lock();
//             let now = Instant::now();
//             (guard.clone(), now)
//         };

//         let time_elapsed = now.duration_since(stats.last_wakeup).as_secs_f64();
//         let utilization = time_elapsed * stats.tickets as f64 + stats.utilization;
//         utilization / now.duration_since(self.start_time).as_secs_f64()
//     }
// }
