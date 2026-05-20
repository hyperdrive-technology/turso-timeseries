//! Informational view SQL for introspection.

/// Hypertable stats view (created by migration 0005).
pub const VIEW_HYPERTABLE_OVERVIEW: &str = "CREATE VIEW IF NOT EXISTS tts_hypertable_overview AS \
SELECT h.hypertable_id, h.table_name, hs.row_count, hs.min_time_micros, hs.max_time_micros, hs.chunk_count, hs.series_count \
FROM _tts_hypertables h \
LEFT JOIN _tts_hypertable_stats hs ON hs.hypertable_id = h.hypertable_id";
