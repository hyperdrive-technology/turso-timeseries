use turso_timeseries_core::model::{FieldValue, TimestampMicros};
use turso_timeseries_core::query::{
    aggregate_finish, aggregate_push, AggregateKind, AggregateValue, DynamicAggregateState,
};

#[test]
fn avg_handles_nulls() {
    let mut state = DynamicAggregateState::new(AggregateKind::Avg);
    aggregate_push(&mut state, &FieldValue::F64(10.0), TimestampMicros(1));
    aggregate_push(&mut state, &FieldValue::Null, TimestampMicros(2));
    aggregate_push(&mut state, &FieldValue::F64(20.0), TimestampMicros(3));
    let finished = aggregate_finish(&state);
    assert!(matches!(finished, AggregateValue::F64(v) if (v - 15.0).abs() < f64::EPSILON));
}
