-- PLAN-v2 catalog extensions: stats, tag index, maintenance jobs.

CREATE TABLE IF NOT EXISTS _tts_hypertable_stats (
  hypertable_id INTEGER PRIMARY KEY REFERENCES _tts_hypertables (hypertable_id) ON DELETE CASCADE,
  row_count INTEGER NOT NULL DEFAULT 0,
  min_time_micros INTEGER,
  max_time_micros INTEGER,
  chunk_count INTEGER NOT NULL DEFAULT 0,
  series_count INTEGER NOT NULL DEFAULT 0,
  updated_at_micros INTEGER NOT NULL DEFAULT (CAST((julianday('now') - 2440587.5) * 86400000000 AS INTEGER))
);

CREATE TABLE IF NOT EXISTS _tts_series_stats (
  series_id INTEGER PRIMARY KEY REFERENCES _tts_series (series_id) ON DELETE CASCADE,
  hypertable_id INTEGER NOT NULL REFERENCES _tts_hypertables (hypertable_id) ON DELETE CASCADE,
  min_time_micros INTEGER,
  max_time_micros INTEGER,
  row_count INTEGER NOT NULL DEFAULT 0,
  chunk_count INTEGER NOT NULL DEFAULT 0,
  updated_at_micros INTEGER NOT NULL DEFAULT (CAST((julianday('now') - 2440587.5) * 86400000000 AS INTEGER))
);

CREATE TABLE IF NOT EXISTS _tts_tag_index (
  hypertable_id INTEGER NOT NULL REFERENCES _tts_hypertables (hypertable_id) ON DELETE CASCADE,
  tag_key TEXT NOT NULL,
  tag_value TEXT NOT NULL,
  series_id INTEGER NOT NULL REFERENCES _tts_series (series_id) ON DELETE CASCADE,
  PRIMARY KEY (hypertable_id, tag_key, tag_value, series_id)
);

CREATE TABLE IF NOT EXISTS _tts_maintenance_jobs (
  job_id INTEGER PRIMARY KEY AUTOINCREMENT,
  job_kind TEXT NOT NULL,
  target_id INTEGER,
  due_at_micros INTEGER NOT NULL,
  locked_at_micros INTEGER,
  completed_at_micros INTEGER,
  error TEXT
);

CREATE INDEX IF NOT EXISTS idx_tts_maintenance_jobs_due
  ON _tts_maintenance_jobs (due_at_micros);

UPDATE _tts_schema_version
SET
  version = MAX(version, 5),
  applied_at = datetime('now')
WHERE
  singleton = 1;
