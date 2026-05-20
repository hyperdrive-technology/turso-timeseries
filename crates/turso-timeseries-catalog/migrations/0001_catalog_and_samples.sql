-- turso-timeseries Phase 1: catalog + raw samples (idempotent DDL).
-- Run after `PRAGMA foreign_keys = ON` on the connection.

CREATE TABLE IF NOT EXISTS _tts_schema_version (
  singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
  version INTEGER NOT NULL,
  applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS _tts_series (
  series_id INTEGER PRIMARY KEY AUTOINCREMENT,
  metric_name TEXT NOT NULL,
  tags_json TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS _tts_samples (
  series_id INTEGER NOT NULL REFERENCES _tts_series (series_id) ON DELETE CASCADE,
  ts_ns INTEGER NOT NULL,
  value_real REAL,
  value_blob BLOB,
  quality INTEGER NOT NULL DEFAULT 0
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_tts_series_metric_tags
  ON _tts_series (metric_name, tags_json);

CREATE UNIQUE INDEX IF NOT EXISTS idx_tts_samples_series_ts_unique
  ON _tts_samples (series_id, ts_ns);

CREATE INDEX IF NOT EXISTS idx_tts_samples_series_ts
  ON _tts_samples (series_id, ts_ns DESC);

CREATE INDEX IF NOT EXISTS idx_tts_samples_ts ON _tts_samples (ts_ns);

INSERT OR IGNORE INTO _tts_schema_version (singleton, version) VALUES (1, 0);

UPDATE _tts_schema_version
SET
  version = MAX(version, 1),
  applied_at = datetime('now')
WHERE
  singleton = 1;
