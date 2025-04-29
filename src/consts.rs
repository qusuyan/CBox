use std::env;
use std::time::Duration;

use lazy_static::lazy_static;

use crate::CopycatError;

lazy_static! {
    pub static ref REPORT_TIME: f64 = env::var("REPORT_PERIOD")
        .map_err(|e| CopycatError(format!("{e:?}")))
        .and_then(|str| str
            .parse::<f64>()
            .map_err(|e| CopycatError(format!("{e:?}"))))
        .unwrap_or(60f64);
    pub static ref REPORT_TIME_INTERVAL: Duration = Duration::from_secs_f64(*REPORT_TIME);
    pub static ref SPIN_WAIT_CUTOFF_MILLIS: u64 = std::env::var("SPIN_WAIT_CUTOFF_MILLIS")
        .map(|s| match s.parse::<u64>() {
            Ok(v) => Some(v),
            Err(e) => {
                log::warn!("Failed to parse SPIN_WAIT_CUTOFF_MILLIS: {e}, use default");
                None
            }
        })
        .unwrap_or(None)
        .unwrap_or(5);
    pub static ref SPIN_WAIT_CUTOFF: Duration = Duration::from_millis(*SPIN_WAIT_CUTOFF_MILLIS);
}

pub const COMMIT_DELAY_INTERVAL: Duration = Duration::from_millis(5);
pub const BLK_DISS_DELAY_INTERVAL: Duration = Duration::from_millis(5);
pub const BLK_MNG_DELAY_INTERVAL: Duration = Duration::from_millis(5);
pub const DECIDE_DELAY_INTERVAL: Duration = Duration::from_millis(5);
pub const PACE_DELAY_INTERVAL: Duration = Duration::from_millis(5);
pub const TXM_DISSEM_DELAY_INTERVAL: Duration = Duration::from_millis(5);
pub const TXN_DISSEM_INTERVAL: Duration = Duration::from_millis(5);
pub const TXN_BATCH_DELAY_INTERVAL: Duration = Duration::from_millis(5);
pub const TXN_BATCH_INTERVAL: Duration = Duration::from_millis(5);
