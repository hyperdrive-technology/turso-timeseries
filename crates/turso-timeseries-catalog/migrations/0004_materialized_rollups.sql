-- turso-timeseries Phase 4: materialized rollups over columnar segments.

CREATE TABLE IF NOT EXISTS _tts_rollups (
  rollup_id INTEGER PRIMARY KEY AUTOINCREMENT,
  source_table TEXT NOT NULL,
  rollup_table TEXT NOT NULL,
  metric_name TEXT NOT NULL,
  tags_json TEXT NOT NULL,
  bucket_ns INTEGER NOT NULL,
  sample_count INTEGER NOT NULL,
  min_value_real REAL,
  max_value_real REAL,
  sum_value_real REAL,
  avg_value_real REAL,
  refreshed_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_tts_rollups_unique_bucket
  ON _tts_rollups (rollup_table, metric_name, tags_json, bucket_ns);

CREATE INDEX IF NOT EXISTS idx_tts_rollups_source_bucket
  ON _tts_rollups (source_table, bucket_ns);

INSERT OR IGNORE INTO _tts_schema_version (singleton, version) VALUES (1, 0);

UPDATE _tts_schema_version
SET
  version = MAX(version, 4),
  applied_at = datetime('now')
WHERE
  singleton = 1;
