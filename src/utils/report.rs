use lazy_static::lazy_static;

use tokio::sync::watch::{Receiver, Sender};
use tokio::task::JoinHandle;
use tokio::time::{interval, Duration, Instant};

const REPORT_TIME_INTERVAL: Duration = Duration::from_secs(60);

lazy_static! {
    static ref CHANNEL: (Sender<Duration>, Receiver<Duration>) =
        tokio::sync::watch::channel(Duration::from_secs(0));
}

// async to ensure it is invoked in a runtime
pub async fn start_report_timer() {
    // initialize timer task once only
    lazy_static! {
        static ref TIMER_TASK: JoinHandle<()> = tokio::spawn(async {
            let sender = &CHANNEL.0;
            let mut timer = interval(REPORT_TIME_INTERVAL);
            let start_time = Instant::now();
            loop {
                timer.tick().await;
                let elapsed = start_time.elapsed();
                if let Err(e) = sender.send(elapsed) {
                    log::error!("Report timer failed unexpectedly: {e}");
                }
            }
        });
    }
}

pub fn get_report_timer() -> Receiver<Duration> {
    CHANNEL.0.subscribe()
}

pub fn get_timer_interval() -> Duration {
    REPORT_TIME_INTERVAL
}
