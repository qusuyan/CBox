use std::sync::OnceLock;

use lazy_static::lazy_static;

use tokio::sync::watch::{Receiver, Sender};
use tokio::task::JoinHandle;
use tokio::time::{interval, Duration, Instant};

const REPORT_TIME_INTERVAL: Duration = Duration::from_secs(10);

lazy_static! {
    static ref CHANNEL: (Sender<Duration>, Receiver<Duration>) =
        tokio::sync::watch::channel(Duration::from_secs(0));
}

// async to ensure it is invoked in a runtime
pub async fn start_report_timer() {
    // initialize timer task once only
    static TIMER_TASK: OnceLock<JoinHandle<()>> = OnceLock::new();

    match TIMER_TASK.get() {
        Some(_) => {
            log::warn!("Report timer has already started");
        }
        None => {
            let task = tokio::spawn(async {
                let sender = &CHANNEL.0;
                let mut timer = interval(REPORT_TIME_INTERVAL);
                timer.tick().await; // first tick occurs immediately
                let start_time = Instant::now();
                loop {
                    timer.tick().await;
                    let elapsed = start_time.elapsed();
                    if let Err(e) = sender.send(elapsed) {
                        log::error!("Report timer failed unexpectedly: {e}");
                    }
                }
            });

            if let Err(e) = TIMER_TASK.set(task) {
                // a concurrent call has already started the timer thread
                // abort the one we just started
                log::warn!("Report timer has already started");
                e.abort();
            }
        }
    }
}

pub fn get_report_timer() -> Receiver<Duration> {
    CHANNEL.0.subscribe()
}

pub fn get_timer_interval() -> Duration {
    REPORT_TIME_INTERVAL
}
