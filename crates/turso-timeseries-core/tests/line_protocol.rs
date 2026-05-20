use turso_timeseries_core::ingest::parse_line_protocol_batch;
use turso_timeseries_core::model::FieldValue;

#[test]
fn parses_measurement_tags_fields_timestamp() {
    let points = parse_line_protocol_batch(
        "metrics,device_id=a value=1.5,flag=true 1778000000000000000",
    )
    .unwrap();
    assert_eq!(points.len(), 1);
    assert_eq!(points[0].table, "metrics");
    assert!(matches!(points[0].fields[0].1, FieldValue::F64(v) if (v - 1.5).abs() < f64::EPSILON));
}
