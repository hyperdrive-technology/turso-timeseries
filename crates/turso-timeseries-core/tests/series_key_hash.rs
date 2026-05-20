use turso_timeseries_core::model::{HypertableId, SeriesKeySpec};

#[test]
fn series_key_hash_is_stable_for_tag_order() {
    let a = SeriesKeySpec::new(
        HypertableId(1),
        [("task", "MainTask"), ("variable", "Motor.Speed")],
    )
    .unwrap();
    let b = SeriesKeySpec::new(
        HypertableId(1),
        [("variable", "Motor.Speed"), ("task", "MainTask")],
    )
    .unwrap();
    assert_eq!(a.series_key_hash(), b.series_key_hash());
}
