use std::time::Duration;

pub const REPORT_TIME_INTERVAL: Duration = Duration::from_secs(60);

pub const COMMIT_DELAY_INTERVAL: Duration = Duration::from_millis(50);
pub const BLK_DISS_DELAY_INTERVAL: Duration = Duration::from_millis(50);
pub const BLK_MNG_DELAY_INTERVAL: Duration = Duration::from_millis(50);
pub const DECIDE_DELAY_INTERVAL: Duration = Duration::from_millis(50);
pub const PACE_DELAY_INTERVAL: Duration = Duration::from_millis(50);
pub const TXM_DISSEM_DELAY_INTERVAL: Duration = Duration::from_millis(50);
pub const TXN_DISSEM_INTERVAL: Duration = Duration::from_millis(100);
pub const TXN_BATCH_DELAY_INTERVAL: Duration = Duration::from_millis(50);
pub const TXN_BATCH_INTERVAL: Duration = Duration::from_millis(100);
