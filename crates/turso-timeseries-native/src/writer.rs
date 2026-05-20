use turso_timeseries_core::codec::encode_segment_v1;
use turso_timeseries_core::ingest::{parse_line_protocol_batch, validate_points};
use turso_timeseries_core::maintenance::invalidate_for_write;
use turso_timeseries_core::model::{ChunkId, FieldValue, MetricPoint, TimestampMicros};

/// Write statistics returned from ingest APIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WriteStats {
    pub points_written: u64,
    pub segments_written: u64,
    pub chunks_touched: u64,
}

pub async fn create_hypertable(
    conn: &turso::Connection,
    table: &str,
    time_column: &str,
    chunk_interval_micros: i64,
) -> turso::Result<i64> {
    conn.execute(
        "INSERT INTO _tts_hypertables (table_name, time_column, chunk_interval_ns, storage_layout) \
         VALUES (?, ?, ?, 'segment_v1') \
         ON CONFLICT(table_name) DO UPDATE SET chunk_interval_ns = excluded.chunk_interval_ns, storage_layout = 'segment_v1'",
        (table, time_column, chunk_interval_micros * 1_000),
    )
    .await?;

    let mut rows = conn
        .query(
            "SELECT hypertable_id FROM _tts_hypertables WHERE table_name = ?",
            (table,),
        )
        .await?;
    let row = rows.next().await?.expect("hypertable row");
    match row.get_value(0)? {
        turso::Value::Integer(id) => Ok(id),
        _ => Ok(0),
    }
}

pub async fn write_line_protocol(conn: &turso::Connection, lines: &str) -> turso::Result<WriteStats> {
    let points = parse_line_protocol_batch(lines).map_err(map_core_err)?;
    if points.is_empty() {
        return Ok(WriteStats::default());
    }
    let table = points[0].table.clone();
    write_metric_points(conn, &table, &points).await
}

pub async fn write_points(
    conn: &turso::Connection,
    table: &str,
    lines: &str,
) -> turso::Result<WriteStats> {
    let mut points = parse_line_protocol_batch(lines).map_err(map_core_err)?;
    for point in &mut points {
        point.table = table.to_string();
    }
    write_metric_points(conn, table, &points).await
}

async fn write_metric_points(
    conn: &turso::Connection,
    table: &str,
    points: &[MetricPoint],
) -> turso::Result<WriteStats> {
    validate_points(points).map_err(map_core_err)?;

    let hypertable_id = ensure_hypertable(conn, table).await?;
    let chunk_interval_ns = hypertable_chunk_interval_ns(conn, table).await?;

    conn.execute("BEGIN", ()).await?;

    let result = write_metric_points_in_tx(conn, hypertable_id, table, chunk_interval_ns, points).await;

    match result {
        Ok(stats) => {
            conn.execute("COMMIT", ()).await?;
            Ok(stats)
        }
        Err(err) => {
            let _ = conn.execute("ROLLBACK", ()).await;
            Err(err)
        }
    }
}

async fn write_metric_points_in_tx(
    conn: &turso::Connection,
    hypertable_id: i64,
    table: &str,
    chunk_interval_ns: i64,
    points: &[MetricPoint],
) -> turso::Result<WriteStats> {
    let mut segments_written = 0u64;
    let mut chunks_touched = 0u64;

    let mut grouped: std::collections::BTreeMap<(String, String, i64), Vec<&MetricPoint>> =
        std::collections::BTreeMap::new();

    for point in points {
        let tags_json = canonical_tags_json(point);
        let chunk_start = point.time.0 * 1_000 - (point.time.0 * 1_000).rem_euclid(chunk_interval_ns);
        grouped
            .entry((point.table.clone(), tags_json, chunk_start))
            .or_default()
            .push(point);
    }

    for ((_, tags_json, chunk_start_ns), group) in grouped {
        let metric_name = group[0].table.as_str();
        let series_id = upsert_series(conn, metric_name, &tags_json).await?;
        upsert_tag_index(conn, hypertable_id, series_id, group[0]).await?;

        let chunk_id = upsert_chunk(
            conn,
            hypertable_id,
            series_id,
            chunk_start_ns,
            chunk_start_ns + chunk_interval_ns,
        )
        .await?;
        chunks_touched += 1;

        let mut times = Vec::new();
        let mut values = Vec::new();
        let mut qualities = Vec::new();
        for point in &group {
            for (_, value) in &point.fields {
                if let FieldValue::F64(v) = value {
                    times.push(point.time.0 * 1_000);
                    values.push(*v);
                    qualities.push(0);
                    break;
                }
            }
        }

        if times.is_empty() {
            continue;
        }

        let payload = encode_segment_v1(ChunkId(chunk_id), &times, &values, &qualities)
            .map_err(map_core_err)?;
        insert_segment_blob(conn, chunk_id, series_id, &times, &values, &payload).await?;
        segments_written += 1;
    }

    if let Some(range) = invalidate_for_write(points) {
        conn.execute(
            "INSERT INTO _tts_invalidations (bucket_start_ns, bucket_end_ns, state) VALUES (?, ?, 'pending')",
            (range.start.0 * 1_000, range.end.0 * 1_000),
        )
        .await?;
        let _ = hypertable_id;
    }

    update_stats(conn, hypertable_id, points.len() as i64).await?;

    Ok(WriteStats {
        points_written: points.len() as u64,
        segments_written,
        chunks_touched,
    })
}

async fn ensure_hypertable(conn: &turso::Connection, table: &str) -> turso::Result<i64> {
    create_hypertable(conn, table, "time", 60 * 60 * 1_000_000).await
}

async fn hypertable_chunk_interval_ns(conn: &turso::Connection, table: &str) -> turso::Result<i64> {
    let mut rows = conn
        .query(
            "SELECT chunk_interval_ns FROM _tts_hypertables WHERE table_name = ?",
            (table,),
        )
        .await?;
    let row = rows.next().await?.expect("hypertable");
    match row.get_value(0)? {
        turso::Value::Integer(v) => Ok(v),
        _ => Ok(60_000_000_000),
    }
}

async fn upsert_series(
    conn: &turso::Connection,
    metric_name: &str,
    tags_json: &str,
) -> turso::Result<i64> {
    conn.execute(
        "INSERT OR IGNORE INTO _tts_series (metric_name, tags_json) VALUES (?, ?)",
        (metric_name, tags_json),
    )
    .await?;
    let mut rows = conn.query(
        "SELECT series_id FROM _tts_series WHERE metric_name = ? AND tags_json = ?",
        (metric_name, tags_json),
    ).await?;
    let row = rows.next().await?.expect("series");
    match row.get_value(0)? {
        turso::Value::Integer(id) => Ok(id),
        _ => Ok(0),
    }
}

async fn upsert_tag_index(
    conn: &turso::Connection,
    hypertable_id: i64,
    series_id: i64,
    point: &MetricPoint,
) -> turso::Result<()> {
    for (key, value) in &point.tags {
        conn.execute(
            "INSERT OR IGNORE INTO _tts_tag_index (hypertable_id, tag_key, tag_value, series_id) VALUES (?, ?, ?, ?)",
            (hypertable_id, key.as_str(), value.as_str(), series_id),
        )
        .await?;
    }
    Ok(())
}

async fn upsert_chunk(
    conn: &turso::Connection,
    hypertable_id: i64,
    series_id: i64,
    chunk_start_ns: i64,
    chunk_end_ns: i64,
) -> turso::Result<i64> {
    conn.execute(
        "INSERT OR IGNORE INTO _tts_chunks (hypertable_id, series_id, chunk_start_ns, chunk_end_ns) VALUES (?, ?, ?, ?)",
        (hypertable_id, series_id, chunk_start_ns, chunk_end_ns),
    )
    .await?;
    let mut rows = conn.query(
        "SELECT chunk_id FROM _tts_chunks WHERE hypertable_id = ? AND series_id = ? AND chunk_start_ns = ?",
        (hypertable_id, series_id, chunk_start_ns),
    ).await?;
    let row = rows.next().await?.expect("chunk");
    match row.get_value(0)? {
        turso::Value::Integer(id) => Ok(id),
        _ => Ok(0),
    }
}

async fn insert_segment_blob(
    conn: &turso::Connection,
    chunk_id: i64,
    series_id: i64,
    times: &[i64],
    values: &[f64],
    payload: &[u8],
) -> turso::Result<()> {
    let start = *times.first().unwrap_or(&0);
    let end = *times.last().unwrap_or(&0);
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let sum: f64 = values.iter().sum();
    let row_count = times.len() as i64;

    conn.execute(
        "INSERT INTO _tts_segments (chunk_id, series_id, segment_start_ns, segment_end_ns, row_count, min_value_real, max_value_real, sum_value_real) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT(chunk_id, series_id, segment_start_ns, segment_end_ns) DO UPDATE SET \
           row_count = excluded.row_count, min_value_real = excluded.min_value_real, max_value_real = excluded.max_value_real, sum_value_real = excluded.sum_value_real",
        (chunk_id, series_id, start, end, row_count, min, max, sum),
    )
    .await?;

    let mut rows = conn.query(
        "SELECT segment_id FROM _tts_segments WHERE chunk_id = ? AND series_id = ? AND segment_start_ns = ? AND segment_end_ns = ?",
        (chunk_id, series_id, start, end),
    ).await?;
    let row = rows.next().await?.expect("segment");
    let segment_id = match row.get_value(0)? {
        turso::Value::Integer(id) => id,
        _ => 0,
    };

    conn.execute(
        "INSERT OR REPLACE INTO _tts_segment_columns (segment_id, column_name, value_type, encoding, data_blob, null_count) VALUES (?, 'payload', 'blob', 'tts_segment_v1', ?, 0)",
        (segment_id, payload.to_vec()),
    )
    .await?;
    Ok(())
}

async fn update_stats(
    conn: &turso::Connection,
    hypertable_id: i64,
    delta_rows: i64,
) -> turso::Result<()> {
    conn.execute(
        "INSERT INTO _tts_hypertable_stats (hypertable_id, row_count, chunk_count, series_count, updated_at_micros) \
         VALUES (?, ?, 0, 0, CAST((julianday('now') - 2440587.5) * 86400000000 AS INTEGER)) \
         ON CONFLICT(hypertable_id) DO UPDATE SET row_count = _tts_hypertable_stats.row_count + excluded.row_count, updated_at_micros = excluded.updated_at_micros",
        (hypertable_id, delta_rows),
    )
    .await?;
    Ok(())
}

fn canonical_tags_json(point: &MetricPoint) -> String {
    let mut pairs: Vec<(String, String)> = point
        .tags
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    pairs.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    let mut json = String::from("{");
    for (i, (key, value)) in pairs.iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        json.push_str(&format!("\"{key}\":\"{value}\""));
    }
    json.push('}');
    json
}

fn map_core_err(err: turso_timeseries_core::Error) -> turso::Error {
    turso::Error::ToSqlConversionFailure(err.to_string().into())
}
