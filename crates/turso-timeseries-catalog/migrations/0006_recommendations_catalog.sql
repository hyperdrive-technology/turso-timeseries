-- RECOMMENDATIONS.md catalog: series tags, ingest WAL, tag stats.

CREATE TABLE IF NOT EXISTS _tts_series_tags (
  series_id INTEGER NOT NULL REFERENCES _tts_series (series_id) ON DELETE CASCADE,
  tag_key TEXT NOT NULL,
  tag_value TEXT NOT NULL,
  PRIMARY KEY (series_id, tag_key)
);

CREATE TABLE IF NOT EXISTS _tts_ingest_wal (
  wal_id INTEGER PRIMARY KEY AUTOINCREMENT,
  hypertable_id INTEGER NOT NULL REFERENCES _tts_hypertables (hypertable_id) ON DELETE CASCADE,
  min_time INTEGER NOT NULL,
  max_time INTEGER NOT NULL,
  row_count INTEGER NOT NULL,
  payload BLOB NOT NULL,
  payload_format TEXT NOT NULL,
  state TEXT NOT NULL DEFAULT 'open',
  created_at INTEGER NOT NULL DEFAULT (CAST((julianday('now') - 2440587.5) * 86400000000 AS INTEGER))
);

CREATE TABLE IF NOT EXISTS _tts_tag_stats (
  hypertable_id INTEGER NOT NULL REFERENCES _tts_hypertables (hypertable_id) ON DELETE CASCADE,
  tag_key TEXT NOT NULL,
  distinct_values INTEGER NOT NULL DEFAULT 0,
  series_count INTEGER NOT NULL DEFAULT 0,
  updated_at INTEGER NOT NULL DEFAULT (CAST((julianday('now') - 2440587.5) * 86400000000 AS INTEGER)),
  PRIMARY KEY (hypertable_id, tag_key)
);

CREATE INDEX IF NOT EXISTS idx_tts_ingest_wal_hypertable_state
  ON _tts_ingest_wal (hypertable_id, state);

UPDATE _tts_schema_version
SET
  version = MAX(version, 6),
  applied_at = datetime('now')
WHERE
  singleton = 1;
