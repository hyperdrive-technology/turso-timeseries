use turso_timeseries_core::model::TimestampMicros;
use turso_timeseries_core::query::MovingAvg;

#[test]
fn moving_average_evicts_old_points() {
    let mut avg = MovingAvg::new(5);
    assert_eq!(avg.push(TimestampMicros(0), 10.0), Some(10.0));
    assert_eq!(avg.push(TimestampMicros(3), 20.0), Some(15.0));
    assert_eq!(avg.push(TimestampMicros(10), 30.0), Some(30.0));
}
