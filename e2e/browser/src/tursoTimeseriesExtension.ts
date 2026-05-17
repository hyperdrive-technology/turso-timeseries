import type { Database } from '@tursodatabase/database-wasm/vite';

export type TimeseriesExtension = {
  extensionLoaded: number;
  timeBucketNs(tsNs: bigint, widthNs: bigint): Promise<bigint>;
};

export async function loadTimeseriesExtension(db: Database): Promise<TimeseriesExtension> {
  await loadTimeseriesSqlExtension(db);
  const loadedRow = (await db.get('SELECT tts_extension_loaded() AS loaded')) as
    | { loaded: number | bigint }
    | undefined;
  const extensionLoaded = Number(loadedRow?.loaded ?? 0);

  return {
    extensionLoaded,
    async timeBucketNs(tsNs, widthNs) {
      return queryTimeBucketNs(db, tsNs, widthNs);
    },
  };
}

export async function loadTimeseriesSqlExtension(db: Database): Promise<void> {
  const bytes = await fetchTimeseriesExtensionBytes();
  const hex = bytesToHex(bytes);

  try {
    await db.exec(`CREATE EXTENSION IF NOT EXISTS turso_timeseries LANGUAGE wasm AS X'${hex}'`);
  } catch (error) {
    throw new Error(
      [
        'failed to load turso-timeseries WASM extension through Turso SQL',
        'This browser E2E requires the custom @tursodatabase/database-wasm build from PR #6256.',
        'Run `npm run build:custom-turso` from e2e/browser, then rerun the tests.',
        error instanceof Error ? error.message : String(error),
      ].join('\n'),
    );
  }
}

export async function queryTimeBucketNs(db: Database, tsNs: bigint, widthNs: bigint): Promise<bigint> {
  const row = (await db.get('SELECT tts_time_bucket_ns(?, ?) AS bucket_ns', tsNs, widthNs)) as
    | { bucket_ns: number | bigint | string }
    | undefined;
  if (!row) {
    throw new Error('tts_time_bucket_ns returned no row');
  }
  return BigInt(String(row.bucket_ns));
}

async function fetchTimeseriesExtensionBytes(): Promise<Uint8Array> {
  const response = await fetch('/tts_extension.wasm');
  if (!response.ok) {
    throw new Error(`failed to fetch extension wasm: ${response.status}`);
  }
  return new Uint8Array(await response.arrayBuffer());
}

function bytesToHex(bytes: Uint8Array): string {
  return [...bytes].map((byte) => byte.toString(16).padStart(2, '0')).join('');
}

export const migrations = [
  `CREATE TABLE IF NOT EXISTS _tts_schema_version (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    version INTEGER NOT NULL,
    applied_at TEXT NOT NULL DEFAULT (datetime('now'))
  )`,
  `CREATE TABLE IF NOT EXISTS _tts_series (
    series_id INTEGER PRIMARY KEY AUTOINCREMENT,
    metric_name TEXT NOT NULL,
    tags_json TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
  )`,
  `CREATE TABLE IF NOT EXISTS _tts_samples (
    series_id INTEGER NOT NULL REFERENCES _tts_series (series_id) ON DELETE CASCADE,
    ts_ns INTEGER NOT NULL,
    value_real REAL,
    value_blob BLOB,
    quality INTEGER NOT NULL DEFAULT 0
  )`,
  `CREATE UNIQUE INDEX IF NOT EXISTS idx_tts_series_metric_tags
    ON _tts_series (metric_name, tags_json)`,
  `CREATE UNIQUE INDEX IF NOT EXISTS idx_tts_samples_series_ts_unique
    ON _tts_samples (series_id, ts_ns)`,
  `CREATE INDEX IF NOT EXISTS idx_tts_samples_series_ts
    ON _tts_samples (series_id, ts_ns DESC)`,
  `CREATE INDEX IF NOT EXISTS idx_tts_samples_ts ON _tts_samples (ts_ns)`,
  `INSERT OR IGNORE INTO _tts_schema_version (singleton, version) VALUES (1, 0)`,
  `UPDATE _tts_schema_version
    SET version = MAX(version, 1), applied_at = datetime('now')
    WHERE singleton = 1`,
  `CREATE TABLE IF NOT EXISTS _tts_retention_policies (
    policy_id INTEGER PRIMARY KEY AUTOINCREMENT,
    target_table TEXT NOT NULL,
    retention_interval_ns INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
  )`,
  `CREATE TABLE IF NOT EXISTS _tts_rollup_policies (
    policy_id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_table TEXT NOT NULL,
    rollup_table TEXT NOT NULL,
    bucket_ns INTEGER NOT NULL,
    aggregates TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
  )`,
  `CREATE TABLE IF NOT EXISTS _tts_rollup_watermarks (
    policy_id INTEGER NOT NULL REFERENCES _tts_rollup_policies (policy_id) ON DELETE CASCADE,
    watermark_ns INTEGER NOT NULL
  )`,
  `CREATE TABLE IF NOT EXISTS _tts_invalidations (
    invalidation_id INTEGER PRIMARY KEY AUTOINCREMENT,
    policy_id INTEGER REFERENCES _tts_rollup_policies (policy_id) ON DELETE SET NULL,
    bucket_start_ns INTEGER NOT NULL,
    bucket_end_ns INTEGER NOT NULL,
    state TEXT NOT NULL DEFAULT 'pending',
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
  )`,
  `CREATE TABLE IF NOT EXISTS _tts_jobs (
    job_id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_type TEXT NOT NULL,
    payload_json TEXT,
    state TEXT NOT NULL DEFAULT 'queued',
    scheduled_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
  )`,
  `CREATE UNIQUE INDEX IF NOT EXISTS idx_tts_retention_policies_target_table
    ON _tts_retention_policies (target_table)`,
  `CREATE UNIQUE INDEX IF NOT EXISTS idx_tts_rollup_watermarks_policy_id
    ON _tts_rollup_watermarks (policy_id)`,
  `CREATE INDEX IF NOT EXISTS idx_tts_invalidations_state ON _tts_invalidations (state)`,
  `CREATE INDEX IF NOT EXISTS idx_tts_jobs_state ON _tts_jobs (state)`,
  `UPDATE _tts_schema_version
    SET version = MAX(version, 2), applied_at = datetime('now')
    WHERE singleton = 1`,
  `CREATE TABLE IF NOT EXISTS _tts_hypertables (
    hypertable_id INTEGER PRIMARY KEY AUTOINCREMENT,
    table_name TEXT NOT NULL,
    time_column TEXT NOT NULL DEFAULT 'ts_ns',
    chunk_interval_ns INTEGER NOT NULL,
    storage_layout TEXT NOT NULL DEFAULT 'columnar',
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
  )`,
  `CREATE TABLE IF NOT EXISTS _tts_chunks (
    chunk_id INTEGER PRIMARY KEY AUTOINCREMENT,
    hypertable_id INTEGER NOT NULL REFERENCES _tts_hypertables (hypertable_id) ON DELETE CASCADE,
    series_id INTEGER REFERENCES _tts_series (series_id) ON DELETE CASCADE,
    chunk_start_ns INTEGER NOT NULL,
    chunk_end_ns INTEGER NOT NULL,
    sealed INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
  )`,
  `CREATE TABLE IF NOT EXISTS _tts_segments (
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
  )`,
  `CREATE TABLE IF NOT EXISTS _tts_segment_columns (
    segment_id INTEGER NOT NULL REFERENCES _tts_segments (segment_id) ON DELETE CASCADE,
    column_name TEXT NOT NULL,
    value_type TEXT NOT NULL,
    encoding TEXT NOT NULL,
    data_blob BLOB NOT NULL,
    null_count INTEGER NOT NULL DEFAULT 0
  )`,
  `CREATE UNIQUE INDEX IF NOT EXISTS idx_tts_hypertables_table_name
    ON _tts_hypertables (table_name)`,
  `CREATE UNIQUE INDEX IF NOT EXISTS idx_tts_chunks_unique_range
    ON _tts_chunks (hypertable_id, series_id, chunk_start_ns)`,
  `CREATE INDEX IF NOT EXISTS idx_tts_chunks_hypertable_range
    ON _tts_chunks (hypertable_id, chunk_start_ns, chunk_end_ns)`,
  `CREATE UNIQUE INDEX IF NOT EXISTS idx_tts_segments_unique_range
    ON _tts_segments (chunk_id, series_id, segment_start_ns, segment_end_ns)`,
  `CREATE INDEX IF NOT EXISTS idx_tts_segments_series_range
    ON _tts_segments (series_id, segment_start_ns, segment_end_ns)`,
  `CREATE UNIQUE INDEX IF NOT EXISTS idx_tts_segment_columns_unique_name
    ON _tts_segment_columns (segment_id, column_name)`,
  `UPDATE _tts_schema_version
    SET version = MAX(version, 3), applied_at = datetime('now')
    WHERE singleton = 1`,
  `CREATE TABLE IF NOT EXISTS _tts_rollups (
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
  )`,
  `CREATE UNIQUE INDEX IF NOT EXISTS idx_tts_rollups_unique_bucket
    ON _tts_rollups (rollup_table, metric_name, tags_json, bucket_ns)`,
  `CREATE INDEX IF NOT EXISTS idx_tts_rollups_source_bucket
    ON _tts_rollups (source_table, bucket_ns)`,
  `UPDATE _tts_schema_version
    SET version = MAX(version, 4), applied_at = datetime('now')
    WHERE singleton = 1`,
];
