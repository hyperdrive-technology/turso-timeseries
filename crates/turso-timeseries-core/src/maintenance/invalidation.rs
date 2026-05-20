#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::model::{MetricPoint, TimestampMicros};

/// Time range for rollup invalidation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeRange {
    pub start: TimestampMicros,
    pub end: TimestampMicros,
}

impl TimeRange {
    #[must_use]
    pub fn width(&self) -> i64 {
        self.end.0 - self.start.0
    }
}

/// Why a range was invalidated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidationReason {
    Write,
    LateData,
    Manual,
}

/// Compute invalidation range for a write batch.
#[must_use]
pub fn invalidate_for_write(points: &[MetricPoint]) -> Option<TimeRange> {
    if points.is_empty() {
        return None;
    }
    let min = points.iter().map(|p| p.time).min()?;
    let max = points.iter().map(|p| p.time).max()?;
    Some(TimeRange { start: min, end: max })
}
