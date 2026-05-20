use turso_timeseries_core::codec::decode_segment_v1;

/// One raw point row decoded from segment storage.
#[derive(Debug, Clone, PartialEq)]
pub struct RawPointRow {
    pub metric_name: String,
    pub tags_json: String,
    pub time_micros: i64,
    pub value: f64,
    pub quality: i32,
}

pub async fn read_points(
    conn: &turso::Connection,
    table: &str,
    from_micros: Option<i64>,
    to_micros: Option<i64>,
) -> turso::Result<Vec<RawPointRow>> {
    scan_raw_rows(conn, table, from_micros, to_micros).await
}

pub async fn scan_raw_rows(
    conn: &turso::Connection,
    table: &str,
    from_micros: Option<i64>,
    to_micros: Option<i64>,
) -> turso::Result<Vec<RawPointRow>> {
    let mut sql = String::from(
        "SELECT s.metric_name, s.tags_json, seg.segment_start_ns, col.data_blob \
         FROM _tts_segments seg \
         JOIN _tts_chunks c ON c.chunk_id = seg.chunk_id \
         JOIN _tts_hypertables h ON h.hypertable_id = c.hypertable_id \
         JOIN _tts_series s ON s.series_id = seg.series_id \
         JOIN _tts_segment_columns col ON col.segment_id = seg.segment_id AND col.column_name = 'payload' \
         WHERE h.table_name = ?",
    );
    if from_micros.is_some() {
        sql.push_str(" AND seg.segment_end_ns >= ?");
    }
    if to_micros.is_some() {
        sql.push_str(" AND seg.segment_start_ns < ?");
    }
    sql.push_str(" ORDER BY s.metric_name, s.tags_json, seg.segment_start_ns");

    let mut params: Vec<turso::Value> = vec![turso::Value::Text(table.to_string())];
    if let Some(v) = from_micros {
        params.push(turso::Value::Integer(v * 1_000));
    }
    if let Some(v) = to_micros {
        params.push(turso::Value::Integer(v * 1_000));
    }

    let mut rows = conn
        .query(&sql, turso::params_from_iter(params))
        .await?;

    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        let metric_name = match row.get_value(0)? {
            turso::Value::Text(s) => s,
            _ => continue,
        };
        let tags_json = match row.get_value(1)? {
            turso::Value::Text(s) => s,
            _ => "{}".to_string(),
        };
        let blob = match row.get_value(3)? {
            turso::Value::Blob(b) => b,
            _ => continue,
        };
        let decoded = decode_segment_v1(&blob).map_err(|e| map_core_err(e))?;
        for point in decoded {
            let micros = point.time.0;
            if from_micros.is_some_and(|start| micros < start) {
                continue;
            }
            if to_micros.is_some_and(|end| micros >= end) {
                continue;
            }
            out.push(RawPointRow {
                metric_name: metric_name.clone(),
                tags_json: tags_json.clone(),
                time_micros: micros,
                value: point.value,
                quality: point.quality,
            });
        }
    }
    Ok(out)
}

fn map_core_err(err: turso_timeseries_core::Error) -> turso::Error {
    turso::Error::ToSqlConversionFailure(err.to_string().into())
}
