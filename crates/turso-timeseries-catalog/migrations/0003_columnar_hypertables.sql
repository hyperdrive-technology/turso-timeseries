-- turso-timeseries Phase 3: columnar hypertable / chunk / segment catalog.

CREATE TABLE IF NOT EXISTS _tts_hypertables (
  hypertable_id INTEGER PRIMARY KEY AUTOINCREMENT,
  table_name TEXT NOT NULL,
  time_column TEXT NOT NULL DEFAULT 'ts_ns',
  chunk_interval_ns INTEGER NOT NULL,
  storage_layout TEXT NOT NULL DEFAULT 'columnar',
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS _tts_chunks (
  chunk_id INTEGER PRIMARY KEY AUTOINCREMENT,
  hypertable_id INTEGER NOT NULL REFERENCES _tts_hypertables (hypertable_id) ON DELETE CASCADE,
  series_id INTEGER REFERENCES _tts_series (series_id) ON DELETE CASCADE,
  chunk_start_ns INTEGER NOT NULL,
  chunk_end_ns INTEGER NOT NULL,
  sealed INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS _tts_segments (
  segment_id INTEGER PRIMARY KEY AUTOINCREMENT,
  chunk_id INTEGER NOT NULL REFERENCES _tts_chunks (chunk_id) ON DELETE CASCADE,
  series_id INTEGER NOT NULL REFERENCES _tts_series (series_id) ON DELETE CASCADE,
  segment_start_ns INTEGER NOT NULL,
  segment_end_ns INTEGER NOT NULL,
  row_count INTEGER NOT NULL,
  min_value_real REAL,
  max_value_real REAL,
  sum_value_real REAL,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS _tts_segment_columns (
  segment_id INTEGER NOT NULL REFERENCES _tts_segments (segment_id) ON DELETE CASCADE,
  column_name TEXT NOT NULL,
  value_type TEXT NOT NULL,
  encoding TEXT NOT NULL,
  data_blob BLOB NOT NULL,
  null_count INTEGER NOT NULL DEFAULT 0
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_tts_hypertables_table_name
  ON _tts_hypertables (table_name);

CREATE UNIQUE INDEX IF NOT EXISTS idx_tts_chunks_unique_range
  ON _tts_chunks (hypertable_id, series_id, chunk_start_ns);

CREATE INDEX IF NOT EXISTS idx_tts_chunks_hypertable_range
  ON _tts_chunks (hypertable_id, chunk_start_ns, chunk_end_ns);

CREATE UNIQUE INDEX IF NOT EXISTS idx_tts_segments_unique_range
  ON _tts_segments (chunk_id, series_id, segment_start_ns, segment_end_ns);

CREATE INDEX IF NOT EXISTS idx_tts_segments_series_range
  ON _tts_segments (series_id, segment_start_ns, segment_end_ns);

CREATE UNIQUE INDEX IF NOT EXISTS idx_tts_segment_columns_unique_name
  ON _tts_segment_columns (segment_id, column_name);

INSERT OR IGNORE INTO _tts_schema_version (singleton, version) VALUES (1, 0);

UPDATE _tts_schema_version
SET
  version = MAX(version, 3),
  applied_at = datetime('now')
WHERE
  singleton = 1;
