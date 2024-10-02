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
}

pub const COMMIT_DELAY_INTERVAL: Duration = Duration::from_millis(50);
pub const BLK_DISS_DELAY_INTERVAL: Duration = Duration::from_millis(50);
pub const BLK_MNG_DELAY_INTERVAL: Duration = Duration::from_millis(50);
pub const DECIDE_DELAY_INTERVAL: Duration = Duration::from_millis(50);
pub const PACE_DELAY_INTERVAL: Duration = Duration::from_millis(50);
pub const TXM_DISSEM_DELAY_INTERVAL: Duration = Duration::from_millis(50);
pub const TXN_DISSEM_INTERVAL: Duration = Duration::from_millis(100);
pub const TXN_BATCH_DELAY_INTERVAL: Duration = Duration::from_millis(50);
pub const TXN_BATCH_INTERVAL: Duration = Duration::from_millis(100);
