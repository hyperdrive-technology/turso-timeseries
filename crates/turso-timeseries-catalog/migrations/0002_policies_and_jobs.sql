-- turso-timeseries Phase 1: retention / rollup metadata + job queue (idempotent DDL).

CREATE TABLE IF NOT EXISTS _tts_retention_policies (
  policy_id INTEGER PRIMARY KEY AUTOINCREMENT,
  target_table TEXT NOT NULL,
  retention_interval_ns INTEGER NOT NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS _tts_rollup_policies (
  policy_id INTEGER PRIMARY KEY AUTOINCREMENT,
  source_table TEXT NOT NULL,
  rollup_table TEXT NOT NULL,
  bucket_ns INTEGER NOT NULL,
  aggregates TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS _tts_rollup_watermarks (
  policy_id INTEGER NOT NULL REFERENCES _tts_rollup_policies (policy_id) ON DELETE CASCADE,
  watermark_ns INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS _tts_invalidations (
  invalidation_id INTEGER PRIMARY KEY AUTOINCREMENT,
  policy_id INTEGER REFERENCES _tts_rollup_policies (policy_id) ON DELETE SET NULL,
  bucket_start_ns INTEGER NOT NULL,
  bucket_end_ns INTEGER NOT NULL,
  state TEXT NOT NULL DEFAULT 'pending',
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS _tts_jobs (
  job_id INTEGER PRIMARY KEY AUTOINCREMENT,
  job_type TEXT NOT NULL,
  payload_json TEXT,
  state TEXT NOT NULL DEFAULT 'queued',
  scheduled_at TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_tts_retention_policies_target_table
  ON _tts_retention_policies (target_table);

CREATE UNIQUE INDEX IF NOT EXISTS idx_tts_rollup_watermarks_policy_id
  ON _tts_rollup_watermarks (policy_id);

CREATE INDEX IF NOT EXISTS idx_tts_invalidations_state ON _tts_invalidations (state);

CREATE INDEX IF NOT EXISTS idx_tts_jobs_state ON _tts_jobs (state);

INSERT OR IGNORE INTO _tts_schema_version (singleton, version) VALUES (1, 0);

UPDATE _tts_schema_version
SET
  version = MAX(version, 2),
  applied_at = datetime('now')
WHERE
  singleton = 1;
