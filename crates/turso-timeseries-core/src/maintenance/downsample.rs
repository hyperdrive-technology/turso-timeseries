use crate::query::{AggregatePlan, AggregateRow, BucketedAggregateExecutor};

/// Report from rollup refresh.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DownsampleReport {
    pub buckets_refreshed: u64,
}

/// Recompute bucket aggregates for dirty ranges.
pub fn refresh_rollup_buckets(
    plan: &AggregatePlan,
    points: &[crate::model::MetricPoint],
) -> (Vec<AggregateRow>, DownsampleReport) {
    let rows = BucketedAggregateExecutor.execute(plan, points);
    let report = DownsampleReport {
        buckets_refreshed: rows.len() as u64,
    };
    (rows, report)
}
