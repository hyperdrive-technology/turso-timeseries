//! Native DB-backed tests. Built only with `--features integration-tests`.

use turso::Builder;
use turso_timeseries::migrations::SCHEMA_VERSION;
use turso_timeseries::native_turso::{apply_migrations, execute_batch, execute_statement};
use turso_timeseries::{
    build_columnar_segments, plan_add_retention_policy, plan_create_rollup_policy,
    plan_delete_columnar_chunks_before, plan_query_columnar_rollup, plan_refresh_columnar_rollup,
    plan_write_batch, plan_write_columnar_segments, MetricPoint, RollupAggregate, SeriesKey,
};

#[tokio::test]
async fn migrations_apply_idempotently_against_native_turso() {
    let db = Builder::new_local(":memory:").build().await.unwrap();
    let conn = db.connect().unwrap();

    apply_migrations(&conn).await.unwrap();
    apply_migrations(&conn).await.unwrap();

    let mut rows = conn
        .query(
            "SELECT version FROM _tts_schema_version WHERE singleton = 1",
            (),
        )
        .await
        .unwrap();
    let row = rows.next().await.unwrap().unwrap();
    assert_eq!(
        row.get_value(0).unwrap(),
        turso::Value::Integer(i64::from(SCHEMA_VERSION))
    );
    assert!(rows.next().await.unwrap().is_none());
}

#[tokio::test]
async fn planned_columnar_segments_write_and_roll_up_against_native_turso() {
    let db = Builder::new_local(":memory:").build().await.unwrap();
    let conn = db.connect().unwrap();
    apply_migrations(&conn).await.unwrap();

    let series = SeriesKey::new("temperature", [("device", "pump-1")]).unwrap();
    let mut points = Vec::new();
    for second in 0..120 {
        points.push(
            MetricPoint::real(
                series.clone(),
                i64::from(second) * 1_000_000_000,
                f64::from(second),
            )
            .unwrap(),
        );
    }

    let segments = build_columnar_segments("_tts_samples", 60_000_000_000, &points).unwrap();
    assert_eq!(segments.len(), 2);

    conn.execute("BEGIN", ()).await.unwrap();
    execute_batch(&conn, &plan_write_columnar_segments(&segments))
        .await
        .unwrap();
    conn.execute("COMMIT", ()).await.unwrap();

    let mut rows = conn
        .query(
            "SELECT column_name, length(data_blob) \
             FROM _tts_segment_columns \
             ORDER BY segment_id, column_name",
            (),
        )
        .await
        .unwrap();

    let first = rows.next().await.unwrap().unwrap();
    assert_eq!(
        first.get_value(0).unwrap(),
        turso::Value::Text("quality".to_string())
    );
    assert_eq!(first.get_value(1).unwrap(), turso::Value::Integer(240));

    let mut rows = conn
        .query(
            "SELECT row_count, min_value_real, max_value_real, sum_value_real \
             FROM _tts_segments \
             ORDER BY segment_start_ns",
            (),
        )
        .await
        .unwrap();
    let first = rows.next().await.unwrap().unwrap();
    assert_eq!(first.get_value(0).unwrap(), turso::Value::Integer(60));
    assert_eq!(first.get_value(1).unwrap(), turso::Value::Real(0.0));
    assert_eq!(first.get_value(2).unwrap(), turso::Value::Real(59.0));
    assert_eq!(first.get_value(3).unwrap(), turso::Value::Real(1770.0));

    let second = rows.next().await.unwrap().unwrap();
    assert_eq!(second.get_value(0).unwrap(), turso::Value::Integer(60));
    assert_eq!(second.get_value(1).unwrap(), turso::Value::Real(60.0));
    assert_eq!(second.get_value(2).unwrap(), turso::Value::Real(119.0));
    assert_eq!(second.get_value(3).unwrap(), turso::Value::Real(5370.0));
    assert!(rows.next().await.unwrap().is_none());

    let rollup_query = plan_query_columnar_rollup("_tts_samples", 60_000_000_000).unwrap();
    let mut rows = conn
        .query(
            rollup_query.sql,
            turso::params_from_iter(
                rollup_query
                    .params
                    .iter()
                    .map(|param| match param {
                        turso_timeseries::SqlValue::Integer(value) => turso::Value::Integer(*value),
                        turso_timeseries::SqlValue::Text(value) => {
                            turso::Value::Text(value.clone())
                        }
                        _ => unreachable!("rollup query only binds integer and text"),
                    })
                    .collect::<Vec<_>>(),
            ),
        )
        .await
        .unwrap();
    let row = rows.next().await.unwrap().unwrap();
    assert_eq!(row.get_value(3).unwrap(), turso::Value::Integer(60));
    assert_eq!(row.get_value(6).unwrap(), turso::Value::Real(29.5));

    conn.execute("BEGIN", ()).await.unwrap();
    execute_batch(
        &conn,
        &plan_refresh_columnar_rollup("_tts_samples", "samples_1m", 60_000_000_000).unwrap(),
    )
    .await
    .unwrap();
    conn.execute("COMMIT", ()).await.unwrap();

    let mut rows = conn
        .query(
            "SELECT bucket_ns, sample_count, min_value_real, max_value_real, avg_value_real \
             FROM _tts_rollups \
             WHERE rollup_table = 'samples_1m' \
             ORDER BY bucket_ns",
            (),
        )
        .await
        .unwrap();
    let first = rows.next().await.unwrap().unwrap();
    assert_eq!(first.get_value(0).unwrap(), turso::Value::Integer(0));
    assert_eq!(first.get_value(1).unwrap(), turso::Value::Integer(60));
    assert_eq!(first.get_value(2).unwrap(), turso::Value::Real(0.0));
    assert_eq!(first.get_value(3).unwrap(), turso::Value::Real(59.0));
    assert_eq!(first.get_value(4).unwrap(), turso::Value::Real(29.5));

    let second = rows.next().await.unwrap().unwrap();
    assert_eq!(
        second.get_value(0).unwrap(),
        turso::Value::Integer(60_000_000_000)
    );
    assert_eq!(second.get_value(1).unwrap(), turso::Value::Integer(60));
    assert_eq!(second.get_value(4).unwrap(), turso::Value::Real(89.5));
    assert!(rows.next().await.unwrap().is_none());

    execute_statement(
        &conn,
        &plan_delete_columnar_chunks_before("_tts_samples", 60_000_000_000).unwrap(),
    )
    .await
    .unwrap();

    let mut rows = conn
        .query("SELECT COUNT(*) FROM _tts_segments", ())
        .await
        .unwrap();
    let row = rows.next().await.unwrap().unwrap();
    assert_eq!(row.get_value(0).unwrap(), turso::Value::Integer(1));

    let mut rows = conn
        .query("SELECT COUNT(*) FROM _tts_segment_columns", ())
        .await
        .unwrap();
    let row = rows.next().await.unwrap().unwrap();
    assert_eq!(row.get_value(0).unwrap(), turso::Value::Integer(3));
}

#[tokio::test]
async fn planned_batch_inserts_and_queries_samples_against_native_turso() {
    let db = Builder::new_local(":memory:").build().await.unwrap();
    let conn = db.connect().unwrap();
    apply_migrations(&conn).await.unwrap();

    let series = SeriesKey::new("temperature", [("device", "pump-1"), ("zone", "a")]).unwrap();
    let points = vec![
        MetricPoint::real(series.clone(), 1_000, 23.5).unwrap(),
        MetricPoint::real(series, 2_000, 24.25)
            .unwrap()
            .with_quality(1),
    ];

    conn.execute("BEGIN", ()).await.unwrap();
    let changed = execute_batch(&conn, &plan_write_batch(&points))
        .await
        .unwrap();
    conn.execute("COMMIT", ()).await.unwrap();

    assert!(
        changed >= 2,
        "Turso should report at least the inserted sample rows"
    );

    let mut rows = conn
        .query(
            "SELECT s.metric_name, s.tags_json, p.ts_ns, p.value_real, p.quality \
             FROM _tts_samples p \
             JOIN _tts_series s ON s.series_id = p.series_id \
             ORDER BY p.ts_ns",
            (),
        )
        .await
        .unwrap();

    let first = rows.next().await.unwrap().unwrap();
    assert_eq!(
        first.get_value(0).unwrap(),
        turso::Value::Text("temperature".to_string())
    );
    assert_eq!(
        first.get_value(1).unwrap(),
        turso::Value::Text("{\"device\":\"pump-1\",\"zone\":\"a\"}".to_string())
    );
    assert_eq!(first.get_value(2).unwrap(), turso::Value::Integer(1_000));
    assert_eq!(first.get_value(3).unwrap(), turso::Value::Real(23.5));
    assert_eq!(first.get_value(4).unwrap(), turso::Value::Integer(0));

    let second = rows.next().await.unwrap().unwrap();
    assert_eq!(second.get_value(2).unwrap(), turso::Value::Integer(2_000));
    assert_eq!(second.get_value(3).unwrap(), turso::Value::Real(24.25));
    assert_eq!(second.get_value(4).unwrap(), turso::Value::Integer(1));
    assert!(rows.next().await.unwrap().is_none());
}

#[tokio::test]
async fn planned_policy_statements_execute_against_native_turso() {
    let db = Builder::new_local(":memory:").build().await.unwrap();
    let conn = db.connect().unwrap();
    apply_migrations(&conn).await.unwrap();

    execute_statement(
        &conn,
        &plan_add_retention_policy("_tts_samples", 86_400_000_000_000).unwrap(),
    )
    .await
    .unwrap();
    execute_statement(
        &conn,
        &plan_create_rollup_policy(
            "_tts_samples",
            "samples_1m",
            60_000_000_000,
            &[
                RollupAggregate::Avg,
                RollupAggregate::Min,
                RollupAggregate::Max,
            ],
        )
        .unwrap(),
    )
    .await
    .unwrap();

    let mut rows = conn
        .query(
            "SELECT target_table, retention_interval_ns FROM _tts_retention_policies",
            (),
        )
        .await
        .unwrap();
    let row = rows.next().await.unwrap().unwrap();
    assert_eq!(
        row.get_value(0).unwrap(),
        turso::Value::Text("_tts_samples".to_string())
    );
    assert_eq!(
        row.get_value(1).unwrap(),
        turso::Value::Integer(86_400_000_000_000)
    );

    let mut rows = conn
        .query(
            "SELECT source_table, rollup_table, bucket_ns, aggregates FROM _tts_rollup_policies",
            (),
        )
        .await
        .unwrap();
    let row = rows.next().await.unwrap().unwrap();
    assert_eq!(
        row.get_value(0).unwrap(),
        turso::Value::Text("_tts_samples".to_string())
    );
    assert_eq!(
        row.get_value(1).unwrap(),
        turso::Value::Text("samples_1m".to_string())
    );
    assert_eq!(
        row.get_value(2).unwrap(),
        turso::Value::Integer(60_000_000_000)
    );
    assert_eq!(
        row.get_value(3).unwrap(),
        turso::Value::Text("avg,max,min".to_string())
    );
}
