//! Placeholder physical plan node for TSDB scans.

#[derive(Debug, Default)]
pub struct TimeseriesScanPlan {
    pub estimated_rows: u64,
}
