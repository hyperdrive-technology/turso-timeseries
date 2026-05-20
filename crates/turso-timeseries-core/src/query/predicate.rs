use crate::model::TimestampMicros;

/// Constraints pushed into scan operators / vtabs.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ScanConstraints {
    pub hypertable: Option<String>,
    pub time_start: Option<TimestampMicros>,
    pub time_end: Option<TimestampMicros>,
    pub series_eq_count: Option<u64>,
    pub field: Option<String>,
}

impl ScanConstraints {
    #[must_use]
    pub fn time_range_micros(&self) -> Option<(i64, i64)> {
        match (self.time_start, self.time_end) {
            (Some(start), Some(end)) => Some((start.0, end.0)),
            _ => None,
        }
    }
}
