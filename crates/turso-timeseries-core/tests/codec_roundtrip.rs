use turso_timeseries_core::codec::{decode_i64_delta_column, encode_i64_delta_column};

#[test]
fn timestamp_delta_roundtrip() {
    let values = vec![1_000, 1_500, 2_000, 9_000];
    let encoded = encode_i64_delta_column(&values);
    let decoded = decode_i64_delta_column(&encoded).unwrap();
    assert_eq!(decoded, values);
}
