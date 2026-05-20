use turso_timeseries_core::query::time_bucket;

#[test]
fn five_minute_boundary() {
    let bucket = time_bucket("5m", 1_778_000_123_456_789).unwrap();
    assert_eq!(bucket, 1_778_000_100_000_000);
}
