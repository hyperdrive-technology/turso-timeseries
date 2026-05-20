use turso_timeseries_core::maintenance::MaintenanceJobKind;

/// Maintenance options (`PLAN-v2.md` §10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MaintenanceOptions {
    pub compact: bool,
    pub downsample: bool,
    pub retention: bool,
    pub refresh_stats: bool,
    pub budget_ms: Option<u64>,
}

/// Maintenance execution report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MaintenanceReport {
    pub jobs_run: u64,
    pub chunks_deleted: u64,
    pub rollups_refreshed: u64,
}

pub async fn run_maintenance(
    conn: &turso::Connection,
    options: MaintenanceOptions,
) -> turso::Result<MaintenanceReport> {
    let started = std::time::Instant::now();
    let mut report = MaintenanceReport::default();

    if options.retention {
        report.chunks_deleted += run_retention(conn).await?;
        report.jobs_run += 1;
    }

    if options.downsample {
        report.rollups_refreshed += refresh_rollups(conn).await?;
        report.jobs_run += 1;
    }

    if options.refresh_stats {
        refresh_hypertable_stats(conn).await?;
        report.jobs_run += 1;
    }

    if options.compact {
        enqueue_job(conn, MaintenanceJobKind::Compact).await?;
        report.jobs_run += 1;
    }

    if let Some(budget_ms) = options.budget_ms {
        if started.elapsed().as_millis() as u64 > budget_ms {
            return Ok(report);
        }
    }

    Ok(report)
}

async fn run_retention(conn: &turso::Connection) -> turso::Result<u64> {
    let mut rows = conn
        .query(
            "SELECT target_table, retention_interval_ns FROM _tts_retention_policies",
            (),
        )
        .await?;

    let mut deleted = 0u64;
    while let Some(row) = rows.next().await? {
        let table = match row.get_value(0)? {
            turso::Value::Text(s) => s,
            _ => continue,
        };
        let interval_ns = match row.get_value(1)? {
            turso::Value::Integer(v) => v,
            _ => continue,
        };
        let cutoff_ns = chrono_like_now_ns() - interval_ns;
        let changed = conn
            .execute(
                "DELETE FROM _tts_chunks WHERE chunk_end_ns <= ? AND hypertable_id IN (SELECT hypertable_id FROM _tts_hypertables WHERE table_name = ?)",
                (cutoff_ns, table),
            )
            .await?;
        deleted += changed;
    }
    Ok(deleted)
}

async fn refresh_rollups(conn: &turso::Connection) -> turso::Result<u64> {
    let mut rows = conn
        .query(
            "SELECT source_table, rollup_table, bucket_ns FROM _tts_rollup_policies",
            (),
        )
        .await?;
    let mut count = 0u64;
    while let Some(row) = rows.next().await? {
        let source = match row.get_value(0)? {
            turso::Value::Text(s) => s,
            _ => continue,
        };
        let rollup = match row.get_value(1)? {
            turso::Value::Text(s) => s,
            _ => continue,
        };
        let bucket_ns = match row.get_value(2)? {
            turso::Value::Integer(v) => v,
            _ => continue,
        };
        conn.execute("BEGIN", ()).await?;
        conn.execute(
            "DELETE FROM _tts_rollups WHERE rollup_table = ?",
            (rollup.clone(),),
        )
        .await?;
        conn.execute(
            "INSERT INTO _tts_rollups (source_table, rollup_table, metric_name, tags_json, bucket_ns, sample_count, min_value_real, max_value_real, sum_value_real, avg_value_real) \
             SELECT ?, ?, s.metric_name, COALESCE(s.tags_json, '{}'), (seg.segment_start_ns - (seg.segment_start_ns % ?)), SUM(seg.row_count), MIN(seg.min_value_real), MAX(seg.max_value_real), SUM(seg.sum_value_real), SUM(seg.sum_value_real) / SUM(seg.row_count) \
             FROM _tts_segments seg \
             JOIN _tts_chunks c ON c.chunk_id = seg.chunk_id \
             JOIN _tts_hypertables h ON h.hypertable_id = c.hypertable_id \
             JOIN _tts_series s ON s.series_id = seg.series_id \
             WHERE h.table_name = ? \
             GROUP BY s.metric_name, tags_json, bucket_ns",
            (source.clone(), rollup, bucket_ns, source),
        )
        .await?;
        conn.execute("COMMIT", ()).await?;
        count += 1;
    }
    Ok(count)
}

async fn refresh_hypertable_stats(conn: &turso::Connection) -> turso::Result<()> {
    conn.execute(
        "INSERT INTO _tts_hypertable_stats (hypertable_id, row_count, chunk_count, series_count, updated_at_micros) \
         SELECT h.hypertable_id, COALESCE(SUM(seg.row_count), 0), COUNT(DISTINCT c.chunk_id), COUNT(DISTINCT s.series_id), CAST((julianday('now') - 2440587.5) * 86400000000 AS INTEGER) \
         FROM _tts_hypertables h \
         LEFT JOIN _tts_chunks c ON c.hypertable_id = h.hypertable_id \
         LEFT JOIN _tts_segments seg ON seg.chunk_id = c.chunk_id \
         LEFT JOIN _tts_series s ON s.series_id = seg.series_id \
         GROUP BY h.hypertable_id \
         ON CONFLICT(hypertable_id) DO UPDATE SET \
           row_count = excluded.row_count, chunk_count = excluded.chunk_count, series_count = excluded.series_count, updated_at_micros = excluded.updated_at_micros",
        (),
    )
    .await?;
    Ok(())
}

async fn enqueue_job(conn: &turso::Connection, kind: MaintenanceJobKind) -> turso::Result<()> {
    let kind_str = match kind {
        MaintenanceJobKind::Compact => "compact",
        MaintenanceJobKind::Downsample => "downsample",
        MaintenanceJobKind::Retention => "retention",
        MaintenanceJobKind::Stats => "stats",
        MaintenanceJobKind::RollupRefresh => "rollup_refresh",
    };
    conn.execute(
        "INSERT INTO _tts_maintenance_jobs (job_kind, due_at_micros) VALUES (?, CAST((julianday('now') - 2440587.5) * 86400000000 AS INTEGER))",
        (kind_str,),
    )
    .await?;
    Ok(())
}

fn chrono_like_now_ns() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    (dur.as_secs() as i64) * 1_000_000_000 + i64::from(dur.subsec_nanos())
}
