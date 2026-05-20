use turso::Builder;
use turso_timeseries_native::Timeseries;

#[tokio::test]
async fn install_write_and_read_line_protocol() {
    let db = Builder::new_local(":memory:").build().await.unwrap();
    let conn = db.connect().unwrap();

    Timeseries::install(&conn).await.unwrap();
    Timeseries::create_hypertable(&conn, "metrics", "time", 60 * 60 * 1_000_000)
        .await
        .unwrap();

    let stats = Timeseries::write_line_protocol(
        &conn,
        "metrics,device_id=a value=1.5 1778000000000000000",
    )
    .await
    .unwrap();
    assert!(stats.points_written >= 1);

    let rows = Timeseries::read_points(&conn, "metrics", None, None)
        .await
        .unwrap();
    assert!(!rows.is_empty());
    assert!((rows[0].value - 1.5).abs() < f64::EPSILON);
}

#[tokio::test]
async fn rollback_hides_uncommitted_writes() {
    let db = Builder::new_local(":memory:").build().await.unwrap();
    let conn = db.connect().unwrap();
    Timeseries::install(&conn).await.unwrap();
    Timeseries::create_hypertable(&conn, "metrics", "time", 60 * 60 * 1_000_000)
        .await
        .unwrap();

    conn.execute("BEGIN", ()).await.unwrap();
    conn.execute(
        "INSERT INTO _tts_series (metric_name, tags_json) VALUES ('metrics', '{\"device_id\":\"a\"}')",
        (),
    )
    .await
    .unwrap();
    conn.execute("ROLLBACK", ()).await.unwrap();

    let mut rows = conn
        .query("SELECT COUNT(*) FROM _tts_series", ())
        .await
        .unwrap();
    let row = rows.next().await.unwrap().unwrap();
    assert_eq!(row.get_value(0).unwrap(), turso::Value::Integer(0));
}
