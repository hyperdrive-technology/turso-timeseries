#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use super::{DurationMicros, HypertableId};

/// Rollup policy linking source and target hypertables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollupPolicy {
    pub source_hypertable_id: HypertableId,
    pub target_hypertable_id: HypertableId,
    pub bucket_width: DurationMicros,
    pub aggregates: Vec<String>,
    pub refresh_lag_micros: i64,
    pub retention_micros: Option<i64>,
    pub enabled: bool,
}
