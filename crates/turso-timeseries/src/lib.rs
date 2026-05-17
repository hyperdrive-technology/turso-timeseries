//! Time-series primitives for use with **[Turso Database](https://github.com/tursodatabase/turso)**
//! (pure Rust, SQLite-compatible). **libSQL is out of scope** for this repo.
//!
//! - **Native:** depend on the **`turso`** crate in your binary.
//! - **`wasm32-unknown-unknown`:** use your **`turso-wasm`** + **`sync-wasm`** build
//!   (Hyperdrive’s wasm Turso + sync stack); this crate stays portable across both.
//!
//! This is **not** a TimescaleDB compatibility layer — only shared *patterns*
//! (bucketing, retention, catalog SQL) you execute through Turso’s Rust API.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub mod migrations;

#[cfg(feature = "native-turso")]
pub mod native_turso;

/// Crate-local result type.
pub type Result<T> = std::result::Result<T, Error>;

/// Error returned by validation and SQL planning helpers.
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// A bucket or retention interval must be strictly positive.
    InvalidInterval { name: &'static str, value: i64 },
    /// Metric names are catalog identifiers and cannot be empty.
    EmptyMetricName,
    /// Tag keys cannot be empty.
    EmptyTagKey,
    /// SQL identifiers accepted by this helper must be simple ASCII identifiers.
    InvalidIdentifier(String),
    /// Rollup policies need at least one aggregate.
    EmptyAggregateList,
    /// Floating point sample values must be finite.
    NonFiniteValue(f64),
    /// Columnar segment planning currently supports real-valued samples only.
    ColumnarRequiresRealValue,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInterval { name, value } => {
                write!(f, "{name} must be positive, got {value}")
            }
            Self::EmptyMetricName => f.write_str("metric_name cannot be empty"),
            Self::EmptyTagKey => f.write_str("tag keys cannot be empty"),
            Self::InvalidIdentifier(identifier) => {
                write!(f, "invalid SQL identifier: {identifier:?}")
            }
            Self::EmptyAggregateList => f.write_str("aggregate list cannot be empty"),
            Self::NonFiniteValue(value) => write!(f, "sample value must be finite, got {value}"),
            Self::ColumnarRequiresRealValue => {
                f.write_str("columnar segment planning requires real-valued samples")
            }
        }
    }
}

impl std::error::Error for Error {}

/// Floor timestamp to a fixed-width bucket in milliseconds (UTC wall-clock math only).
pub fn time_bucket_ms(ts_ms: i64, width_ms: i64) -> Result<i64> {
    time_bucket(ts_ms, width_ms, "width_ms")
}

/// Floor timestamp to a fixed-width bucket in nanoseconds.
pub fn time_bucket_ns(ts_ns: i64, width_ns: i64) -> Result<i64> {
    time_bucket(ts_ns, width_ns, "width_ns")
}

fn time_bucket(ts: i64, width: i64, name: &'static str) -> Result<i64> {
    if width <= 0 {
        return Err(Error::InvalidInterval { name, value: width });
    }
    Ok(ts - ts.rem_euclid(width))
}

/// A deterministic series identity: metric name plus sorted tags.
///
/// `tags_json` is canonicalized by this crate so `_tts_series` uniqueness works
/// consistently across native and browser callers.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SeriesKey {
    metric_name: String,
    tags_json: String,
}

impl SeriesKey {
    /// Create a key from a metric name and tag pairs.
    pub fn new<I, K, V>(metric_name: impl Into<String>, tags: I) -> Result<Self>
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let metric_name = metric_name.into();
        if metric_name.trim().is_empty() {
            return Err(Error::EmptyMetricName);
        }

        let mut tags_vec = Vec::new();
        for (key, value) in tags {
            let key = key.into();
            if key.is_empty() {
                return Err(Error::EmptyTagKey);
            }
            tags_vec.push((key, value.into()));
        }
        tags_vec.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

        Ok(Self {
            metric_name,
            tags_json: canonical_tags_json(&tags_vec),
        })
    }

    /// Metric name as stored in `_tts_series.metric_name`.
    #[must_use]
    pub fn metric_name(&self) -> &str {
        &self.metric_name
    }

    /// Canonical JSON object string stored in `_tts_series.tags_json`.
    #[must_use]
    pub fn tags_json(&self) -> &str {
        &self.tags_json
    }
}

/// Sample value supported by the conservative row layout.
#[derive(Debug, Clone, PartialEq)]
pub enum MetricValue {
    /// Numeric sample stored in `_tts_samples.value_real`.
    Real(f64),
    /// Binary or encoded value stored in `_tts_samples.value_blob`.
    Blob(Vec<u8>),
}

impl MetricValue {
    /// Build a finite real-valued sample.
    pub fn real(value: f64) -> Result<Self> {
        if !value.is_finite() {
            return Err(Error::NonFiniteValue(value));
        }
        Ok(Self::Real(value))
    }

    /// Build a blob-valued sample.
    #[must_use]
    pub fn blob(value: impl Into<Vec<u8>>) -> Self {
        Self::Blob(value.into())
    }
}

/// A single sample ready for the v1 row layout.
#[derive(Debug, Clone, PartialEq)]
pub struct MetricPoint {
    /// Series identity.
    pub series: SeriesKey,
    /// Timestamp in nanoseconds.
    pub ts_ns: i64,
    /// Sample value.
    pub value: MetricValue,
    /// Optional sample quality/status code. `0` means normal by convention.
    pub quality: i32,
}

impl MetricPoint {
    /// Create a real-valued point with default quality.
    pub fn real(series: SeriesKey, ts_ns: i64, value: f64) -> Result<Self> {
        Ok(Self {
            series,
            ts_ns,
            value: MetricValue::real(value)?,
            quality: 0,
        })
    }

    /// Create a blob-valued point with default quality.
    #[must_use]
    pub fn blob(series: SeriesKey, ts_ns: i64, value: impl Into<Vec<u8>>) -> Self {
        Self {
            series,
            ts_ns,
            value: MetricValue::blob(value),
            quality: 0,
        }
    }

    /// Override the quality/status code.
    #[must_use]
    pub fn with_quality(mut self, quality: i32) -> Self {
        self.quality = quality;
        self
    }
}

/// Bind value for dependency-free SQL planning.
#[derive(Debug, Clone, PartialEq)]
pub enum SqlValue {
    /// SQL NULL.
    Null,
    /// Integer bind.
    Integer(i64),
    /// Floating point bind.
    Real(f64),
    /// Text bind.
    Text(String),
    /// Blob bind.
    Blob(Vec<u8>),
}

/// One SQL statement and its positional bind values.
#[derive(Debug, Clone, PartialEq)]
pub struct SqlStatement {
    /// SQL text using `?` placeholders.
    pub sql: &'static str,
    /// Positional bind values.
    pub params: Vec<SqlValue>,
}

/// A batch of SQL statements to execute in order inside a transaction.
#[derive(Debug, Clone, PartialEq)]
pub struct SqlBatch {
    /// Ordered statements.
    pub statements: Vec<SqlStatement>,
}

impl SqlBatch {
    /// Returns true when there is no work to execute.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.statements.is_empty()
    }
}

/// In-memory representation of one columnar real-valued segment.
///
/// The segment stores each logical column in its own blob (`ts_ns`, `value_real`,
/// `quality`) before writing to `_tts_segment_columns`.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnarSegment {
    /// Source hypertable/catalog name, usually `_tts_samples`.
    pub table_name: String,
    /// Series identity.
    pub series: SeriesKey,
    /// Chunk width in nanoseconds.
    pub chunk_interval_ns: i64,
    /// Chunk start in nanoseconds.
    pub chunk_start_ns: i64,
    /// Chunk end in nanoseconds (exclusive).
    pub chunk_end_ns: i64,
    /// First sample timestamp in this segment.
    pub segment_start_ns: i64,
    /// Last sample timestamp in this segment.
    pub segment_end_ns: i64,
    /// Number of rows encoded in this segment.
    pub row_count: i64,
    /// Little-endian i64 timestamp column.
    pub ts_ns_blob: Vec<u8>,
    /// Little-endian f64 real value column.
    pub value_real_blob: Vec<u8>,
    /// Little-endian i32 quality column.
    pub quality_blob: Vec<u8>,
    /// Minimum real value in the segment.
    pub min_value_real: f64,
    /// Maximum real value in the segment.
    pub max_value_real: f64,
    /// Sum of real values in the segment.
    pub sum_value_real: f64,
}

/// Build one real-valued columnar segment per `(series, chunk)` group.
pub fn build_columnar_segments(
    table_name: &str,
    chunk_interval_ns: i64,
    points: &[MetricPoint],
) -> Result<Vec<ColumnarSegment>> {
    validate_identifier(table_name)?;
    if chunk_interval_ns <= 0 {
        return Err(Error::InvalidInterval {
            name: "chunk_interval_ns",
            value: chunk_interval_ns,
        });
    }

    let mut grouped: BTreeMap<(SeriesKey, i64), Vec<&MetricPoint>> = BTreeMap::new();
    for point in points {
        if !matches!(point.value, MetricValue::Real(_)) {
            return Err(Error::ColumnarRequiresRealValue);
        }
        let chunk_start_ns = time_bucket_ns(point.ts_ns, chunk_interval_ns)?;
        grouped
            .entry((point.series.clone(), chunk_start_ns))
            .or_default()
            .push(point);
    }

    let mut segments = Vec::new();
    for ((series, chunk_start_ns), mut grouped_points) in grouped {
        grouped_points.sort_by_key(|point| point.ts_ns);

        let mut ts_ns = Vec::with_capacity(grouped_points.len());
        let mut values = Vec::with_capacity(grouped_points.len());
        let mut qualities = Vec::with_capacity(grouped_points.len());

        for point in grouped_points {
            let MetricValue::Real(value) = point.value else {
                return Err(Error::ColumnarRequiresRealValue);
            };
            ts_ns.push(point.ts_ns);
            values.push(value);
            qualities.push(point.quality);
        }

        let min_value_real = values.iter().copied().fold(f64::INFINITY, f64::min);
        let max_value_real = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let sum_value_real = values.iter().sum();

        segments.push(ColumnarSegment {
            table_name: table_name.to_string(),
            series,
            chunk_interval_ns,
            chunk_start_ns,
            chunk_end_ns: chunk_start_ns + chunk_interval_ns,
            segment_start_ns: *ts_ns.first().expect("grouped segments are nonempty"),
            segment_end_ns: *ts_ns.last().expect("grouped segments are nonempty"),
            row_count: ts_ns.len() as i64,
            ts_ns_blob: encode_i64_le_column(&ts_ns),
            value_real_blob: encode_f64_le_column(&values),
            quality_blob: encode_i32_le_column(&qualities),
            min_value_real,
            max_value_real,
            sum_value_real,
        });
    }

    Ok(segments)
}

/// Plan idempotent writes for `_tts_hypertables`, `_tts_chunks`, `_tts_segments`
/// and `_tts_segment_columns`.
#[must_use]
pub fn plan_write_columnar_segments(segments: &[ColumnarSegment]) -> SqlBatch {
    const UPSERT_HYPERTABLE: &str = "INSERT INTO _tts_hypertables (table_name, time_column, chunk_interval_ns, storage_layout) VALUES (?, 'ts_ns', ?, 'columnar') ON CONFLICT(table_name) DO UPDATE SET chunk_interval_ns = excluded.chunk_interval_ns, storage_layout = 'columnar'";
    const UPSERT_SERIES: &str =
        "INSERT OR IGNORE INTO _tts_series (metric_name, tags_json) VALUES (?, ?)";
    const UPSERT_CHUNK: &str = "INSERT OR IGNORE INTO _tts_chunks (hypertable_id, series_id, chunk_start_ns, chunk_end_ns) SELECT h.hypertable_id, s.series_id, ?, ? FROM _tts_hypertables h JOIN _tts_series s ON s.metric_name = ? AND s.tags_json = ? WHERE h.table_name = ?";
    const UPSERT_SEGMENT: &str = "INSERT INTO _tts_segments (chunk_id, series_id, segment_start_ns, segment_end_ns, row_count, min_value_real, max_value_real, sum_value_real) SELECT c.chunk_id, s.series_id, ?, ?, ?, ?, ?, ? FROM _tts_chunks c JOIN _tts_hypertables h ON h.hypertable_id = c.hypertable_id JOIN _tts_series s ON s.series_id = c.series_id WHERE h.table_name = ? AND s.metric_name = ? AND s.tags_json = ? AND c.chunk_start_ns = ? ON CONFLICT(chunk_id, series_id, segment_start_ns, segment_end_ns) DO UPDATE SET row_count = excluded.row_count, min_value_real = excluded.min_value_real, max_value_real = excluded.max_value_real, sum_value_real = excluded.sum_value_real";
    const UPSERT_COLUMN: &str = "INSERT OR REPLACE INTO _tts_segment_columns (segment_id, column_name, value_type, encoding, data_blob, null_count) SELECT seg.segment_id, ?, ?, ?, ?, 0 FROM _tts_segments seg JOIN _tts_chunks c ON c.chunk_id = seg.chunk_id JOIN _tts_hypertables h ON h.hypertable_id = c.hypertable_id JOIN _tts_series s ON s.series_id = seg.series_id WHERE h.table_name = ? AND s.metric_name = ? AND s.tags_json = ? AND c.chunk_start_ns = ? AND seg.segment_start_ns = ? AND seg.segment_end_ns = ?";

    let mut statements = Vec::new();
    let mut hypertables = BTreeSet::new();
    let mut series = BTreeSet::new();
    let mut chunks = BTreeSet::new();

    for segment in segments {
        if hypertables.insert((segment.table_name.clone(), segment.chunk_interval_ns)) {
            statements.push(SqlStatement {
                sql: UPSERT_HYPERTABLE,
                params: vec![
                    SqlValue::Text(segment.table_name.clone()),
                    SqlValue::Integer(segment.chunk_interval_ns),
                ],
            });
        }
        if series.insert(segment.series.clone()) {
            statements.push(SqlStatement {
                sql: UPSERT_SERIES,
                params: vec![
                    SqlValue::Text(segment.series.metric_name().to_string()),
                    SqlValue::Text(segment.series.tags_json().to_string()),
                ],
            });
        }
    }

    for segment in segments {
        let chunk_key = (
            segment.table_name.clone(),
            segment.series.clone(),
            segment.chunk_start_ns,
        );
        if chunks.insert(chunk_key) {
            statements.push(SqlStatement {
                sql: UPSERT_CHUNK,
                params: vec![
                    SqlValue::Integer(segment.chunk_start_ns),
                    SqlValue::Integer(segment.chunk_end_ns),
                    SqlValue::Text(segment.series.metric_name().to_string()),
                    SqlValue::Text(segment.series.tags_json().to_string()),
                    SqlValue::Text(segment.table_name.clone()),
                ],
            });
        }

        statements.push(SqlStatement {
            sql: UPSERT_SEGMENT,
            params: vec![
                SqlValue::Integer(segment.segment_start_ns),
                SqlValue::Integer(segment.segment_end_ns),
                SqlValue::Integer(segment.row_count),
                SqlValue::Real(segment.min_value_real),
                SqlValue::Real(segment.max_value_real),
                SqlValue::Real(segment.sum_value_real),
                SqlValue::Text(segment.table_name.clone()),
                SqlValue::Text(segment.series.metric_name().to_string()),
                SqlValue::Text(segment.series.tags_json().to_string()),
                SqlValue::Integer(segment.chunk_start_ns),
            ],
        });

        for (column_name, value_type, encoding, data_blob) in [
            ("ts_ns", "integer", "i64-le", segment.ts_ns_blob.clone()),
            (
                "value_real",
                "real",
                "f64-le",
                segment.value_real_blob.clone(),
            ),
            ("quality", "integer", "i32-le", segment.quality_blob.clone()),
        ] {
            statements.push(SqlStatement {
                sql: UPSERT_COLUMN,
                params: vec![
                    SqlValue::Text(column_name.to_string()),
                    SqlValue::Text(value_type.to_string()),
                    SqlValue::Text(encoding.to_string()),
                    SqlValue::Blob(data_blob),
                    SqlValue::Text(segment.table_name.clone()),
                    SqlValue::Text(segment.series.metric_name().to_string()),
                    SqlValue::Text(segment.series.tags_json().to_string()),
                    SqlValue::Integer(segment.chunk_start_ns),
                    SqlValue::Integer(segment.segment_start_ns),
                    SqlValue::Integer(segment.segment_end_ns),
                ],
            });
        }
    }

    SqlBatch { statements }
}

/// Plan a columnar hypertable catalog upsert without writing segments yet.
pub fn plan_create_hypertable(table_name: &str, chunk_interval_ns: i64) -> Result<SqlStatement> {
    validate_identifier(table_name)?;
    if chunk_interval_ns <= 0 {
        return Err(Error::InvalidInterval {
            name: "chunk_interval_ns",
            value: chunk_interval_ns,
        });
    }

    Ok(SqlStatement {
        sql: "INSERT INTO _tts_hypertables (table_name, time_column, chunk_interval_ns, storage_layout) VALUES (?, 'ts_ns', ?, 'columnar') ON CONFLICT(table_name) DO UPDATE SET chunk_interval_ns = excluded.chunk_interval_ns, storage_layout = 'columnar'",
        params: vec![
            SqlValue::Text(table_name.to_string()),
            SqlValue::Integer(chunk_interval_ns),
        ],
    })
}

/// Plan a bounded retention delete for columnar chunks. Segment rows and column
/// blobs cascade through foreign keys.
pub fn plan_delete_columnar_chunks_before(
    table_name: &str,
    older_than_ns: i64,
) -> Result<SqlStatement> {
    validate_identifier(table_name)?;

    Ok(SqlStatement {
        sql: "DELETE FROM _tts_chunks WHERE chunk_end_ns <= ? AND hypertable_id IN (SELECT hypertable_id FROM _tts_hypertables WHERE table_name = ?)",
        params: vec![
            SqlValue::Integer(older_than_ns),
            SqlValue::Text(table_name.to_string()),
        ],
    })
}

/// Query aligned columnar segment stats as downsampled rollups.
pub fn plan_query_columnar_rollup(table_name: &str, bucket_ns: i64) -> Result<SqlStatement> {
    validate_identifier(table_name)?;
    if bucket_ns <= 0 {
        return Err(Error::InvalidInterval {
            name: "bucket_ns",
            value: bucket_ns,
        });
    }

    Ok(SqlStatement {
        sql: "SELECT s.metric_name, s.tags_json, (seg.segment_start_ns - (seg.segment_start_ns % ?)) AS bucket_ns, SUM(seg.row_count) AS sample_count, MIN(seg.min_value_real) AS min_value_real, MAX(seg.max_value_real) AS max_value_real, SUM(seg.sum_value_real) / SUM(seg.row_count) AS avg_value_real FROM _tts_segments seg JOIN _tts_chunks c ON c.chunk_id = seg.chunk_id JOIN _tts_hypertables h ON h.hypertable_id = c.hypertable_id JOIN _tts_series s ON s.series_id = seg.series_id WHERE h.table_name = ? GROUP BY s.metric_name, s.tags_json, bucket_ns ORDER BY s.metric_name, s.tags_json, bucket_ns",
        params: vec![SqlValue::Integer(bucket_ns), SqlValue::Text(table_name.to_string())],
    })
}

/// Plan a full refresh of a materialized rollup table from columnar segment stats.
///
/// This is intentionally conservative: delete the named rollup and rebuild it
/// from `_tts_segments` in one caller-owned transaction. Later workers can
/// narrow this to dirty invalidation windows.
pub fn plan_refresh_columnar_rollup(
    source_table: &str,
    rollup_table: &str,
    bucket_ns: i64,
) -> Result<SqlBatch> {
    validate_identifier(source_table)?;
    validate_identifier(rollup_table)?;
    if bucket_ns <= 0 {
        return Err(Error::InvalidInterval {
            name: "bucket_ns",
            value: bucket_ns,
        });
    }

    Ok(SqlBatch {
        statements: vec![
            SqlStatement {
                sql: "DELETE FROM _tts_rollups WHERE rollup_table = ?",
                params: vec![SqlValue::Text(rollup_table.to_string())],
            },
            SqlStatement {
                sql: "INSERT INTO _tts_rollups (source_table, rollup_table, metric_name, tags_json, bucket_ns, sample_count, min_value_real, max_value_real, sum_value_real, avg_value_real) SELECT ?, ?, s.metric_name, COALESCE(s.tags_json, '{}') AS tags_json, (seg.segment_start_ns - (seg.segment_start_ns % ?)) AS bucket_ns, SUM(seg.row_count) AS sample_count, MIN(seg.min_value_real) AS min_value_real, MAX(seg.max_value_real) AS max_value_real, SUM(seg.sum_value_real) AS sum_value_real, SUM(seg.sum_value_real) / SUM(seg.row_count) AS avg_value_real FROM _tts_segments seg JOIN _tts_chunks c ON c.chunk_id = seg.chunk_id JOIN _tts_hypertables h ON h.hypertable_id = c.hypertable_id JOIN _tts_series s ON s.series_id = seg.series_id WHERE h.table_name = ? GROUP BY s.metric_name, tags_json, bucket_ns",
                params: vec![
                    SqlValue::Text(source_table.to_string()),
                    SqlValue::Text(rollup_table.to_string()),
                    SqlValue::Integer(bucket_ns),
                    SqlValue::Text(source_table.to_string()),
                ],
            },
        ],
    })
}

/// Encode an i64 column as packed little-endian values.
#[must_use]
pub fn encode_i64_le_column(values: &[i64]) -> Vec<u8> {
    let mut out = Vec::with_capacity(values.len() * std::mem::size_of::<i64>());
    for value in values {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

/// Encode an i32 column as packed little-endian values.
#[must_use]
pub fn encode_i32_le_column(values: &[i32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(values.len() * std::mem::size_of::<i32>());
    for value in values {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

/// Encode an f64 column as packed little-endian IEEE-754 values.
#[must_use]
pub fn encode_f64_le_column(values: &[f64]) -> Vec<u8> {
    let mut out = Vec::with_capacity(values.len() * std::mem::size_of::<f64>());
    for value in values {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

/// Build idempotent insert statements for the v1 `_tts_series` + `_tts_samples` layout.
#[must_use]
pub fn plan_write_batch(points: &[MetricPoint]) -> SqlBatch {
    const UPSERT_SERIES: &str =
        "INSERT OR IGNORE INTO _tts_series (metric_name, tags_json) VALUES (?, ?)";
    const UPSERT_SAMPLE: &str = "INSERT OR REPLACE INTO _tts_samples (series_id, ts_ns, value_real, value_blob, quality) SELECT series_id, ?, ?, ?, ? FROM _tts_series WHERE metric_name = ? AND tags_json = ?";

    let mut seen_series = BTreeSet::new();
    let mut statements = Vec::new();

    for point in points {
        if seen_series.insert(point.series.clone()) {
            statements.push(SqlStatement {
                sql: UPSERT_SERIES,
                params: vec![
                    SqlValue::Text(point.series.metric_name().to_string()),
                    SqlValue::Text(point.series.tags_json().to_string()),
                ],
            });
        }
    }

    for point in points {
        let (value_real, value_blob) = match &point.value {
            MetricValue::Real(value) => (SqlValue::Real(*value), SqlValue::Null),
            MetricValue::Blob(value) => (SqlValue::Null, SqlValue::Blob(value.clone())),
        };
        statements.push(SqlStatement {
            sql: UPSERT_SAMPLE,
            params: vec![
                SqlValue::Integer(point.ts_ns),
                value_real,
                value_blob,
                SqlValue::Integer(i64::from(point.quality)),
                SqlValue::Text(point.series.metric_name().to_string()),
                SqlValue::Text(point.series.tags_json().to_string()),
            ],
        });
    }

    SqlBatch { statements }
}

/// Aggregates supported by v1 rollup policy metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RollupAggregate {
    /// Number of samples.
    Count,
    /// Sum of real-valued samples.
    Sum,
    /// Average of real-valued samples.
    Avg,
    /// Minimum real-valued sample.
    Min,
    /// Maximum real-valued sample.
    Max,
    /// First sample by timestamp.
    First,
    /// Last sample by timestamp.
    Last,
}

impl RollupAggregate {
    /// Stable SQL/catalog spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Count => "count",
            Self::Sum => "sum",
            Self::Avg => "avg",
            Self::Min => "min",
            Self::Max => "max",
            Self::First => "first",
            Self::Last => "last",
        }
    }
}

/// Build the catalog representation for a rollup aggregate set.
pub fn encode_rollup_aggregates(aggregates: &[RollupAggregate]) -> Result<String> {
    if aggregates.is_empty() {
        return Err(Error::EmptyAggregateList);
    }
    let mut unique = BTreeSet::new();
    for aggregate in aggregates {
        unique.insert(aggregate.as_str());
    }
    Ok(unique.into_iter().collect::<Vec<_>>().join(","))
}

/// Plan an upsert into `_tts_retention_policies`.
pub fn plan_add_retention_policy(
    target_table: &str,
    retention_interval_ns: i64,
) -> Result<SqlStatement> {
    validate_identifier(target_table)?;
    if retention_interval_ns <= 0 {
        return Err(Error::InvalidInterval {
            name: "retention_interval_ns",
            value: retention_interval_ns,
        });
    }
    Ok(SqlStatement {
        sql: "INSERT INTO _tts_retention_policies (target_table, retention_interval_ns) VALUES (?, ?) ON CONFLICT(target_table) DO UPDATE SET retention_interval_ns = excluded.retention_interval_ns",
        params: vec![
            SqlValue::Text(target_table.to_string()),
            SqlValue::Integer(retention_interval_ns),
        ],
    })
}

/// Plan an insert into `_tts_rollup_policies`.
pub fn plan_create_rollup_policy(
    source_table: &str,
    rollup_table: &str,
    bucket_ns: i64,
    aggregates: &[RollupAggregate],
) -> Result<SqlStatement> {
    validate_identifier(source_table)?;
    validate_identifier(rollup_table)?;
    if bucket_ns <= 0 {
        return Err(Error::InvalidInterval {
            name: "bucket_ns",
            value: bucket_ns,
        });
    }

    Ok(SqlStatement {
        sql: "INSERT INTO _tts_rollup_policies (source_table, rollup_table, bucket_ns, aggregates) VALUES (?, ?, ?, ?)",
        params: vec![
            SqlValue::Text(source_table.to_string()),
            SqlValue::Text(rollup_table.to_string()),
            SqlValue::Integer(bucket_ns),
            SqlValue::Text(encode_rollup_aggregates(aggregates)?),
        ],
    })
}

fn validate_identifier(identifier: &str) -> Result<()> {
    let mut chars = identifier.chars();
    match chars.next() {
        Some(c) if c == '_' || c.is_ascii_alphabetic() => {}
        _ => return Err(Error::InvalidIdentifier(identifier.to_string())),
    }
    if chars.all(|c| c == '_' || c.is_ascii_alphanumeric()) {
        Ok(())
    } else {
        Err(Error::InvalidIdentifier(identifier.to_string()))
    }
}

fn canonical_tags_json(tags: &[(String, String)]) -> String {
    let mut json = String::from("{");
    for (i, (key, value)) in tags.iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        json.push('"');
        push_json_escaped(&mut json, key);
        json.push_str("\":\"");
        push_json_escaped(&mut json, value);
        json.push('"');
    }
    json.push('}');
    json
}

fn push_json_escaped(out: &mut String, value: &str) {
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if c < ' ' => {
                out.push_str("\\u");
                out.push_str(&format!("{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bucket_aligns() {
        assert_eq!(time_bucket_ms(3_600_000, 3_600_000), Ok(3_600_000));
        assert_eq!(time_bucket_ms(3_600_001, 3_600_000), Ok(3_600_000));
        assert_eq!(time_bucket_ms(-500, 1_000), Ok(-1_000));
        assert_eq!(time_bucket_ns(1_500, 1_000), Ok(1_000));
    }

    #[test]
    fn bucket_rejects_non_positive_width() {
        assert_eq!(
            time_bucket_ms(1, 0),
            Err(Error::InvalidInterval {
                name: "width_ms",
                value: 0
            })
        );
    }

    #[test]
    fn series_key_canonicalizes_tags() {
        let a = SeriesKey::new("temp", [("zone", "a"), ("device", "pump-1")]).unwrap();
        let b = SeriesKey::new("temp", [("device", "pump-1"), ("zone", "a")]).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.metric_name(), "temp");
        assert_eq!(a.tags_json(), "{\"device\":\"pump-1\",\"zone\":\"a\"}");
    }

    #[test]
    fn series_key_escapes_tags() {
        let key = SeriesKey::new("temp", [("a\"b", "line\none")]).unwrap();
        assert_eq!(key.tags_json(), "{\"a\\\"b\":\"line\\none\"}");
    }

    #[test]
    fn series_key_uses_empty_json_for_no_tags() {
        let key = SeriesKey::new("temp", std::iter::empty::<(&str, &str)>()).unwrap();
        assert_eq!(key.tags_json(), "{}");
    }

    #[test]
    fn real_values_must_be_finite() {
        assert!(matches!(
            MetricValue::real(f64::NAN),
            Err(Error::NonFiniteValue(value)) if value.is_nan()
        ));
    }

    #[test]
    fn write_batch_deduplicates_series_then_writes_samples() {
        let series = SeriesKey::new("temp", [("device", "pump-1")]).unwrap();
        let points = vec![
            MetricPoint::real(series.clone(), 100, 23.5).unwrap(),
            MetricPoint::real(series.clone(), 200, 24.0)
                .unwrap()
                .with_quality(1),
        ];

        let batch = plan_write_batch(&points);
        assert_eq!(batch.statements.len(), 3);
        assert_eq!(
            batch.statements[0].params,
            vec![
                SqlValue::Text("temp".to_string()),
                SqlValue::Text("{\"device\":\"pump-1\"}".to_string())
            ]
        );
        assert_eq!(
            batch.statements[2].params,
            vec![
                SqlValue::Integer(200),
                SqlValue::Real(24.0),
                SqlValue::Null,
                SqlValue::Integer(1),
                SqlValue::Text("temp".to_string()),
                SqlValue::Text("{\"device\":\"pump-1\"}".to_string())
            ]
        );
    }

    #[test]
    fn write_batch_plans_blob_values() {
        let series = SeriesKey::new("event", std::iter::empty::<(&str, &str)>()).unwrap();
        let batch = plan_write_batch(&[MetricPoint::blob(series, 10, [1, 2, 3])]);
        assert_eq!(
            batch.statements[1].params[1..3],
            [SqlValue::Null, SqlValue::Blob(vec![1, 2, 3])]
        );
    }

    #[test]
    fn rollup_aggregates_are_canonicalized() {
        assert_eq!(
            encode_rollup_aggregates(&[
                RollupAggregate::Avg,
                RollupAggregate::Count,
                RollupAggregate::Avg,
            ]),
            Ok("avg,count".to_string())
        );
    }

    #[test]
    fn policy_helpers_validate_inputs() {
        assert!(plan_add_retention_policy("samples", 1_000).is_ok());
        assert!(plan_add_retention_policy("bad-name", 1_000).is_err());
        assert!(plan_create_rollup_policy(
            "samples",
            "samples_5m",
            300_000_000_000,
            &[RollupAggregate::Avg, RollupAggregate::Min]
        )
        .is_ok());
        assert!(
            plan_create_rollup_policy("samples", "samples_5m", 0, &[RollupAggregate::Avg]).is_err()
        );
    }

    #[test]
    fn columnar_segments_group_points_by_series_and_chunk() {
        let series = SeriesKey::new("temp", [("device", "pump-1")]).unwrap();
        let points = vec![
            MetricPoint::real(series.clone(), 0, 10.0).unwrap(),
            MetricPoint::real(series.clone(), 30_000_000_000, 20.0)
                .unwrap()
                .with_quality(2),
            MetricPoint::real(series, 60_000_000_000, 30.0).unwrap(),
        ];

        let segments = build_columnar_segments("_tts_samples", 60_000_000_000, &points).unwrap();

        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].chunk_start_ns, 0);
        assert_eq!(segments[0].row_count, 2);
        assert_eq!(segments[0].min_value_real, 10.0);
        assert_eq!(segments[0].max_value_real, 20.0);
        assert_eq!(segments[0].sum_value_real, 30.0);
        assert_eq!(segments[0].ts_ns_blob.len(), 2 * std::mem::size_of::<i64>());
        assert_eq!(
            segments[0].value_real_blob.len(),
            2 * std::mem::size_of::<f64>()
        );
        assert_eq!(
            segments[0].quality_blob.len(),
            2 * std::mem::size_of::<i32>()
        );

        assert_eq!(segments[1].chunk_start_ns, 60_000_000_000);
        assert_eq!(segments[1].row_count, 1);
        assert_eq!(segments[1].sum_value_real, 30.0);
    }

    #[test]
    fn columnar_segments_reject_blob_values() {
        let series = SeriesKey::new("event", std::iter::empty::<(&str, &str)>()).unwrap();
        let err = build_columnar_segments(
            "_tts_samples",
            60_000_000_000,
            &[MetricPoint::blob(series, 0, [1, 2, 3])],
        )
        .unwrap_err();

        assert_eq!(err, Error::ColumnarRequiresRealValue);
    }

    #[test]
    fn columnar_write_plan_writes_segment_columns_separately() {
        let series = SeriesKey::new("temp", [("device", "pump-1")]).unwrap();
        let points = vec![
            MetricPoint::real(series.clone(), 0, 10.0).unwrap(),
            MetricPoint::real(series, 1_000_000_000, 11.0).unwrap(),
        ];
        let segments = build_columnar_segments("_tts_samples", 60_000_000_000, &points).unwrap();
        let batch = plan_write_columnar_segments(&segments);

        assert!(batch
            .statements
            .iter()
            .any(|statement| statement.sql.contains("_tts_hypertables")));

        let column_writes = batch
            .statements
            .iter()
            .filter(|statement| statement.sql.contains("_tts_segment_columns"))
            .collect::<Vec<_>>();
        assert_eq!(column_writes.len(), 3);
        assert_eq!(
            column_writes[0].params[0],
            SqlValue::Text("ts_ns".to_string())
        );
        assert_eq!(
            column_writes[1].params[0],
            SqlValue::Text("value_real".to_string())
        );
        assert_eq!(
            column_writes[2].params[0],
            SqlValue::Text("quality".to_string())
        );
        assert!(matches!(column_writes[0].params[3], SqlValue::Blob(_)));
    }

    #[test]
    fn columnar_hypertable_planners_validate_inputs() {
        assert!(plan_create_hypertable("_tts_samples", 60_000_000_000).is_ok());
        assert!(plan_create_hypertable("bad-name", 60_000_000_000).is_err());
        assert!(plan_create_hypertable("_tts_samples", 0).is_err());
        assert!(plan_delete_columnar_chunks_before("_tts_samples", 123).is_ok());
        assert!(plan_query_columnar_rollup("_tts_samples", 60_000_000_000).is_ok());
        assert!(plan_query_columnar_rollup("_tts_samples", -1).is_err());
        assert!(plan_refresh_columnar_rollup("_tts_samples", "samples_1m", 60_000_000_000).is_ok());
        assert!(plan_refresh_columnar_rollup("_tts_samples", "bad-name", 60_000_000_000).is_err());
    }
}
