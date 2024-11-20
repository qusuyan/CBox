use super::CopycatError;

use tokio::sync::Notify;
use tokio::task::JoinHandle;
use tokio::time::{interval, Duration, Instant};

use std::collections::VecDeque;
use std::fmt::Debug;
use std::sync::{Arc, LazyLock, OnceLock};

#[derive(Debug)]
struct TimeEvent<T> {
    data: T,
    time: Instant,
}

impl<T> PartialEq for TimeEvent<T> {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time
    }
}

impl<T> Eq for TimeEvent<T> {}

impl<T> Ord for TimeEvent<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if self.time < other.time {
            std::cmp::Ordering::Greater
        } else if self.time == other.time {
            std::cmp::Ordering::Equal
        } else {
            std::cmp::Ordering::Less
        }
    }
}

impl<T> PartialOrd for TimeEvent<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

pub struct TimeQueue<T> {
    inner: VecDeque<TimeEvent<T>>,
    notify: Arc<Notify>,
}

impl<T> TimeQueue<T> {
    pub fn new(check_time: Duration) -> Self {
        static NOTIFY: LazyLock<Arc<Notify>> = LazyLock::new(|| Arc::new(Notify::new()));
        static _WAKER: OnceLock<JoinHandle<()>> = OnceLock::new();

        if _WAKER.get().is_none() {
            let notify_clone = NOTIFY.clone();
            let task = tokio::spawn(async move {
                let mut timer = interval(check_time);
                loop {
                    timer.tick().await;
                    notify_clone.notify_waiters();
                }
            });
            if let Err(e) = _WAKER.set(task) {
                // a concurrent call has already started the timer thread
                // abort the one we just started
                log::warn!("Report timer has already started");
                e.abort();
            }
        }

        Self {
            inner: VecDeque::new(),
            notify: NOTIFY.clone(),
        }
    }

    pub fn push(&mut self, data: T, time: Instant) {
        let mut insert_idx = self.inner.len();
        for idx in (0..self.inner.len()).rev() {
            if self.inner[idx].time > time {
                insert_idx = idx;
            } else {
                break;
            }
        }
        self.inner.insert(insert_idx, TimeEvent { data, time });
    }

    pub fn pop(&mut self) -> Option<T> {
        self.inner.pop_front().map(|te| te.data)
    }

    pub async fn wait_next(&self) -> Result<(), CopycatError> {
        loop {
            match self.inner.front() {
                Some(te) => {
                    if te.time < Instant::now() {
                        return Ok(());
                    }
                }
                None => return Err(CopycatError(String::from("waiting on empty time queue"))),
            }
            self.notify.notified().await;
        }
    }
}

impl<T: Eq + Debug> TimeQueue<T> {
    pub fn remove(&mut self, data: &T) -> Option<T> {
        let mut remove_idx = None;
        for idx in 0..self.inner.len() {
            let te = &self.inner[idx];
            if te.data == *data {
                remove_idx = Some(idx);
                break;
            }
        }

        remove_idx
            .and_then(|idx| self.inner.remove(idx))
            .map(|te| te.data)
    }
}
