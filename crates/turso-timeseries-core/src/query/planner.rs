#[cfg(not(feature = "std"))]
use alloc::{collections::BTreeMap, string::String, vec::Vec};
#[cfg(feature = "std")]
use std::collections::BTreeMap;

use crate::model::{FieldValue, MetricPoint, SeriesKey, TimestampMicros};

use super::aggregate::{
    aggregate_finish, aggregate_push,     aggregate_states_from_names, AggregateKind, AggregateValue,
};
use super::time_bucket::time_bucket_micros;

/// Scan plan for raw points.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanPlan {
    pub hypertable: String,
    pub time_start: Option<TimestampMicros>,
    pub time_end: Option<TimestampMicros>,
}

/// Bucketed aggregate query plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregatePlan {
    pub hypertable: String,
    pub bucket_width_micros: i64,
    pub aggregates: Vec<&'static str>,
}

/// Executes bucketed aggregation over in-memory points (native/tests).
#[derive(Debug, Default)]
pub struct BucketedAggregateExecutor;

impl BucketedAggregateExecutor {
    pub fn execute(
        &self,
        plan: &AggregatePlan,
        points: &[MetricPoint],
    ) -> Vec<AggregateRow> {
        let mut grouped: BTreeMap<(SeriesKey, i64), Vec<(&MetricPoint, &FieldValue)>> =
            BTreeMap::new();

        for point in points {
            if point.table != plan.hypertable {
                continue;
            }
            let bucket = time_bucket_micros(plan.bucket_width_micros, point.time.0)
                .unwrap_or(point.time.0);
            let tag_refs: Vec<(&str, &str)> = point
                .tags
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();
            let series = SeriesKey::new(&point.table, &tag_refs).unwrap_or_else(|_| {
                SeriesKey::new("_unknown", &[]).unwrap()
            });
            if let Some((_, value)) = point.fields.first() {
                grouped
                    .entry((series, bucket))
                    .or_default()
                    .push((point, value));
            }
        }

        let mut out = Vec::new();
        for ((series, bucket), rows) in grouped {
            let mut states = aggregate_states_from_names(&plan.aggregates);
            for (point, value) in &rows {
                for (_, state) in &mut states {
                    aggregate_push(state, value, point.time);
                }
            }
            let values = states
                .iter()
                .map(|(kind, state)| (*kind, aggregate_finish(state)))
                .collect();
            out.push(AggregateRow {
                series,
                bucket_micros: bucket,
                values,
            });
        }
        out
    }
}

/// One aggregated bucket row.
#[derive(Debug, Clone, PartialEq)]
pub struct AggregateRow {
    pub series: SeriesKey,
    pub bucket_micros: i64,
    pub values: Vec<(AggregateKind, AggregateValue)>,
}
