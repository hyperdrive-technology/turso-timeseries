# PLAN-v2.md — `turso-timeseries`

Status: draft v2  
Repository target: `hyperdrive-technology/turso-timeseries`  
Primary goal: a pure-Rust, Turso-native time-series extension with a browser-compatible WASM extension artifact and a native Rust integration artifact.

---

## 0. Executive summary

`PLAN-v2` replaces the earlier “generic Rust/WASM TSDB helper” direction with a sharper implementation target:

1. **Native Turso extension path:** a Rust/native integration that works with the `turso` crate in Rust/native deployments.
2. **Browser-native Turso extension path:** a raw Turso WASM extension artifact following Turso PR #6256’s `CREATE FUNCTION ... LANGUAGE wasm` / `CREATE EXTENSION ... LANGUAGE wasm` design.
3. **Core TSDB engine:** a shared pure-Rust core, designed to be `no_std + alloc` compatible where possible, containing codecs, time-series models, aggregation state machines, rollup logic, and query planning primitives.
4. **Storage model:** TSDB catalog and segment payloads stored inside Turso tables/BLOBs first, rather than sidecar files, so that MVCC, local persistence, OPFS-backed browser databases, and sync can treat TSDB state as ordinary database state.
5. **Aggregation/window strategy:** do not wait for generic WASM aggregate/window function support. Implement aggregate/window-like behaviour inside the `turso-timeseries` hypertable/query engine and expose it through scalar functions, virtual tables, table-valued query APIs, materialized rollups, and explicit query operators.
6. **Join strategy:** expose constraint-aware virtual tables for raw scans, aggregate scans, rollups, latest values, chunks, and series metadata. Push down time/tag/series constraints and maintain statistics to guide join cost/row estimates.
7. **Sync:** keep sync orchestration out of the main repo for now. `turso-timeseries` should be local/native/browser-capable without depending on sync packages. A later `turso-timeseries-sync` repo can own push/pull/checkpoint orchestration and sync-aware compaction policy.

---

## 1. Source references reviewed

This plan is based on the current design discussion and these public/reference materials:

- Turso PR #6256: **WASM user-defined functions and extensions**.
  - Adds `CREATE FUNCTION name LANGUAGE wasm AS X'...' EXPORT 'export_name';`
  - Adds `CREATE EXTENSION name LANGUAGE wasm AS X'...';`
  - Single UDF ABI: `(argc: i32, argv: i32) -> i64`
  - Required exports: `memory`, `turso_malloc`, function exports.
  - Full extension ABI: `turso_ext_init` returning JSON manifest with `functions`, `types`, and `vtabs`.
  - Explicitly unstable; aggregate/window WASM functions are not yet available.
- Turso PR #6256 docs/examples:
  - `wasm-sdk/examples/rust/src/lib.rs`
  - `docs/sql-reference/src/statements/create-function.md`
  - `docs/sql-reference/src/statements/create-extension.md`
- SQLite virtual table model:
  - `xBestIndex`/constraint pushdown/costing concepts.
- TimescaleDB concepts:
  - hypertables
  - chunks
  - `time_bucket`
  - continuous aggregates
  - retention/compression policies
  - informational views
- InfluxDB concepts:
  - TSM-style WAL/cache/immutable segment architecture
  - series/tag model
  - compaction
  - downsampling/retention pattern
- InfluxDB 3 concepts:
  - Rust
  - Arrow/DataFusion/Parquet inspiration
  - separation of router/ingester/querier/catalog/compactor concepts
- WebAssembly/WASI distinction:
  - a Turso extension is a host-specific WASM ABI, not a WASI/WASIX app.

---

## 2. Current diff against original plan

The “original plan” is reconstructed from the first-pass direction discussed before this v2 plan.

| Area | Original plan | PLAN-v2 decision | Reason |
|---|---|---|---|
| Repository shape | Possibly broad monorepo with runtime/browser/sync pieces | Focused `hyperdrive-technology/turso-timeseries` repo only | Keep this repo product-focused and reusable |
| Turso dependency | Possibly vendor Turso crates into same repo | Do **not** vendor Turso | Use Turso as upstream dependency/target runtime; follow extension ABI |
| Browser WASM | WASI/WASIX considered for compatibility | Use raw Turso WASM extension ABI, not WASI/WASIX | PR #6256 expects specific exported symbols and manifest, not POSIX-like syscalls |
| Browser helper | `wasm-bindgen` considered as extension path | `wasm-bindgen` only for optional JS helper, not the extension artifact | Turso WASM extension ABI is not the wasm-bindgen ABI |
| Extension path | General WASM module | Dedicated `turso-timeseries-wasm-ext` crate | Cleanly targets `CREATE EXTENSION ... LANGUAGE wasm` |
| Aggregates | Hope for aggregate/window UDF support | Implement aggregate/window semantics internally via vtabs/query APIs/rollups | PR #6256 does not yet support aggregate/window WASM functions |
| Joins | Unclear | Constraint-aware vtabs + stats + rollup vtabs | Needed for complex joins with normal Turso tables |
| Storage | Maybe sidecar TSM files | Store segments as Turso BLOBs first | Preserves Turso MVCC, OPFS, sync compatibility |
| Sync | Might be in same package | Separate future repo/package | Sync has distinct dependency/release/policy concerns |
| DataFusion/Arrow | Possible core dependencies | Optional native/server adapters only | Too heavy for browser/WASM v1 |
| Timescale parity | Broad parity ambition | Tiered implementation: ergonomic subset first | Full parity is too large for v1 |
| Influx reuse | Maybe direct InfluxDB crates | Borrow concepts; direct reuse only if small/portable | InfluxDB is server-shaped; Turso extension needs tighter ABI |

---

## 3. Product definition

` t u r s o - t i m e s e r i e s ` is a pure-Rust time-series layer for Turso.

It provides:

- TimescaleDB-inspired:
  - hypertables
  - chunks
  - `time_bucket`
  - rollups/continuous-aggregate-like refresh
  - retention policies
  - informational views
- InfluxDB-inspired:
  - line protocol ingest
  - series/tag/field model
  - WAL-like append path
  - TSM-style segment encoding
  - compaction
  - downsampling
- Turso-native:
  - catalog tables inside Turso
  - segment payloads inside Turso BLOBs
  - MVCC-safe batch visibility
  - browser-native extension artifact
  - future sync compatibility
  - OPFS compatibility through Turso’s browser database layer
- Query engine:
  - scalar helper functions
  - constraint-aware virtual tables
  - explicit TSDB query APIs
  - materialized rollup hypertables
  - internal aggregate/window state machines

---

## 4. Non-goals for v1

The following are intentionally not v1 goals:

- Full TimescaleDB compatibility.
- Full InfluxDB compatibility.
- Generic SQL aggregate UDF registration through Turso WASM.
- Generic SQL window UDF registration through Turso WASM.
- WASIX runtime dependency.
- WASI filesystem dependency.
- Sidecar `.tsm` files.
- Required Arrow/DataFusion dependency.
- Remote-only `@tursodatabase/serverless` path.
- Sync orchestration inside the main package.
- Full SQL planner integration/rewrite engine.

---

## 5. Target artifacts

### 5.1 Rust release

Crate:

```text
turso-timeseries
```

Expected use:

```rust
use turso_timeseries::Timeseries;

let db = turso::Builder::new_local("metrics.db").build().await?;
let conn = db.connect().await?;

Timeseries::install(&conn).await?;
Timeseries::create_hypertable(&conn, "metrics", "time").await?;
Timeseries::write_line_protocol(&conn, "metrics,device_id=a value=1 1778000000000000000").await?;
```

### 5.2 WASM extension release

Artifact:

```text
turso_timeseries_ext.wasm
```

Expected use:

```sql
CREATE EXTENSION turso_timeseries
LANGUAGE wasm
AS X'<hex-encoded-turso_timeseries_ext.wasm>';
```

Browser use:

```ts
import { connect } from "@tursodatabase/database-wasm";

const db = await connect("metrics.db");
const wasm = await fetch("/turso_timeseries_ext.wasm").then(r => r.arrayBuffer());
const hex = [...new Uint8Array(wasm)].map(b => b.toString(16).padStart(2, "0")).join("");

await db.exec(`
  CREATE EXTENSION turso_timeseries
  LANGUAGE wasm
  AS X'${hex}'
`);
```

### 5.3 Optional JS helper package

Package:

```text
@hyperdrive-technology/turso-timeseries
```

Purpose:

- hex loading helper
- SQL installer wrapper
- typed `createHypertable`
- typed `writeLineProtocol`
- typed `queryAggregate`
- browser smoke-test helpers

It does **not** replace the Turso WASM extension ABI.

---

## 6. Repository structure

```text
turso-timeseries/
  Cargo.toml
  Cargo.lock
  rust-toolchain.toml
  README.md
  PLAN-v2.md
  LICENSE
  justfile

  crates/
    turso-timeseries-core/
      Cargo.toml
      src/
        lib.rs
        error.rs

        model/
          mod.rs
          time.rs
          value.rs
          point.rs
          series.rs
          schema.rs
          hypertable.rs
          chunk.rs
          segment.rs
          rollup.rs

        ingest/
          mod.rs
          batch.rs
          line_protocol.rs
          validator.rs
          write_buffer.rs

        codec/
          mod.rs
          segment_format.rs
          block.rs
          time_delta.rs
          float_xor.rs
          int_delta.rs
          dictionary.rs
          bitmap.rs
          checksum.rs

        query/
          mod.rs
          predicate.rs
          planner.rs
          stats.rs
          scan.rs
          aggregate.rs
          window.rs
          time_bucket.rs
          last_value.rs

        maintenance/
          mod.rs
          compact.rs
          downsample.rs
          retention.rs
          invalidation.rs
          job.rs

        storage/
          mod.rs
          catalog.rs
          segment_store.rs
          transaction.rs

      tests/
        codec_roundtrip.rs
        line_protocol.rs
        time_bucket.rs
        aggregate_states.rs
        window_states.rs
        downsample.rs
        compaction.rs

    turso-timeseries-catalog/
      Cargo.toml
      src/
        lib.rs
        migrations.rs
        schema.rs
        views.rs

    turso-timeseries-native/
      Cargo.toml
      src/
        lib.rs
        install.rs
        connection.rs
        catalog_store.rs
        segment_store.rs
        writer.rs
        query.rs
        maintenance.rs
        vtab.rs
      tests/
        install.rs
        write_read.rs
        transaction_visibility.rs
        vtab_join.rs
        maintenance.rs

    turso-timeseries-wasm-ext/
      Cargo.toml
      src/
        lib.rs
        manifest.rs
        exports.rs
        scalar.rs
        vtab.rs
        panic.rs
      examples/
        load_extension.sql

    turso-timeseries-arrow/
      Cargo.toml
      src/
        lib.rs
        record_batch.rs
        schema_map.rs

    turso-timeseries-datafusion/
      Cargo.toml
      src/
        lib.rs
        table_provider.rs
        physical_plan.rs

  packages/
    turso-timeseries-js/
      package.json
      tsconfig.json
      src/
        index.ts
        loadExtension.ts
        install.ts
        hypertable.ts
        ingest.ts
        query.ts
        maintenance.ts
      test/
        browser-local.spec.ts
        browser-vtab.spec.ts

  examples/
    rust-local/
      Cargo.toml
      src/main.rs

    rust-native-extension/
      Cargo.toml
      src/main.rs

    browser-vite/
      package.json
      vite.config.ts
      src/main.ts

    browser-create-extension/
      package.json
      src/main.ts

  tests/
    fixtures/
      line_protocol/
      query/
      rollups/
      wasm/

  benches/
    ingest.rs
    query.rs
    compaction.rs
    downsample.rs
```

---

## 7. Cargo workspace

```toml
[workspace]
resolver = "2"
members = [
  "crates/turso-timeseries-core",
  "crates/turso-timeseries-catalog",
  "crates/turso-timeseries-native",
  "crates/turso-timeseries-wasm-ext",
  "crates/turso-timeseries-arrow",
  "crates/turso-timeseries-datafusion",
  "examples/rust-local",
]

[workspace.dependencies]
anyhow = "1"
thiserror = "2"
bytes = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
smallvec = "1"
bitvec = "1"
hashbrown = "0.15"
crc32fast = "1"

# Native integration.
turso = "0.x"
tokio = { version = "1", features = ["rt-multi-thread", "macros", "time", "sync"] }

# WASM extension ABI. Initially likely pinned to Turso PR branch / eventual SDK crate.
turso-wasm-sdk = { git = "https://github.com/tursodatabase/turso", branch = "turso-wasm-udf", package = "turso-wasm-sdk" }

# Optional analytics.
arrow = { version = "x", optional = true }
datafusion = { version = "x", optional = true }
```

---

## 8. Core crate design

### 8.1 Core rules

`turso-timeseries-core` should:

- avoid Turso dependency
- avoid `wasm-bindgen`
- avoid WASI/WASIX
- avoid OPFS/fetch/networking
- avoid DataFusion dependency
- support `no_std + alloc` where practical
- own data model, codecs, query planning primitives, aggregate/window state machines, and maintenance algorithms

### 8.2 Core feature flags

```toml
[features]
default = ["std", "line-protocol"]
std = []
alloc = []
line-protocol = []
simd = []
zstd = []
arrow = []
datafusion = ["arrow"]
```

### 8.3 Core data model

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimestampMicros(pub i64);

#[derive(Debug, Clone)]
pub enum FieldValue {
    Null,
    I64(i64),
    F64(f64),
    Bool(bool),
    Text(alloc::string::String),
    Blob(alloc::vec::Vec<u8>),
}

#[derive(Debug, Clone)]
pub struct MetricPoint {
    pub table: alloc::string::String,
    pub time: TimestampMicros,
    pub tags: alloc::vec::Vec<(alloc::string::String, alloc::string::String)>,
    pub fields: alloc::vec::Vec<(alloc::string::String, FieldValue)>,
}
```

### 8.4 Hypertable model

```rust
pub struct Hypertable {
    pub id: HypertableId,
    pub name: String,
    pub time_column: String,
    pub tag_columns: Vec<String>,
    pub field_columns: Vec<FieldColumn>,
    pub chunk_interval: DurationMicros,
}

pub struct ChunkMeta {
    pub id: ChunkId,
    pub hypertable_id: HypertableId,
    pub time_start: TimestampMicros,
    pub time_end: TimestampMicros,
    pub sealed: bool,
    pub level: u32,
    pub row_count: u64,
    pub stats: ChunkStats,
}
```

---

## 9. Turso catalog schema

The first implementation stores TSDB state inside Turso tables.

```sql
CREATE TABLE IF NOT EXISTS _tts_schema_version (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  version INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS _tts_hypertables (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  time_column TEXT NOT NULL,
  chunk_interval_micros INTEGER NOT NULL,
  created_at_micros INTEGER NOT NULL,
  config_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS _tts_columns (
  hypertable_id INTEGER NOT NULL,
  name TEXT NOT NULL,
  logical_type TEXT NOT NULL,
  role TEXT NOT NULL, -- time | tag | field
  nullable INTEGER NOT NULL DEFAULT 1,
  PRIMARY KEY (hypertable_id, name)
);

CREATE TABLE IF NOT EXISTS _tts_chunks (
  id INTEGER PRIMARY KEY,
  hypertable_id INTEGER NOT NULL,
  time_start_micros INTEGER NOT NULL,
  time_end_micros INTEGER NOT NULL,
  partition_key TEXT,
  sealed INTEGER NOT NULL DEFAULT 0,
  level INTEGER NOT NULL DEFAULT 0,
  row_count INTEGER NOT NULL DEFAULT 0,
  minmax_json TEXT,
  created_at_micros INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS _tts_segments (
  id INTEGER PRIMARY KEY,
  chunk_id INTEGER NOT NULL,
  segment_kind TEXT NOT NULL, -- wal | tsm | rollup | index
  sequence_no INTEGER NOT NULL,
  codec TEXT NOT NULL,
  data BLOB NOT NULL,
  row_count INTEGER NOT NULL,
  min_time_micros INTEGER NOT NULL,
  max_time_micros INTEGER NOT NULL,
  checksum BLOB,
  created_at_micros INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS _tts_series (
  id INTEGER PRIMARY KEY,
  hypertable_id INTEGER NOT NULL,
  series_key_hash BLOB NOT NULL,
  tags_json TEXT NOT NULL,
  UNIQUE (hypertable_id, series_key_hash)
);

CREATE TABLE IF NOT EXISTS _tts_tag_index (
  hypertable_id INTEGER NOT NULL,
  tag_key TEXT NOT NULL,
  tag_value TEXT NOT NULL,
  series_id INTEGER NOT NULL,
  PRIMARY KEY (hypertable_id, tag_key, tag_value, series_id)
);

CREATE TABLE IF NOT EXISTS _tts_invalidations (
  id INTEGER PRIMARY KEY,
  hypertable_id INTEGER NOT NULL,
  time_start_micros INTEGER NOT NULL,
  time_end_micros INTEGER NOT NULL,
  reason TEXT NOT NULL,
  created_at_micros INTEGER NOT NULL,
  processed_at_micros INTEGER
);

CREATE TABLE IF NOT EXISTS _tts_rollup_policies (
  id INTEGER PRIMARY KEY,
  source_hypertable_id INTEGER NOT NULL,
  target_hypertable_id INTEGER NOT NULL,
  bucket_width_micros INTEGER NOT NULL,
  aggregates_json TEXT NOT NULL,
  refresh_lag_micros INTEGER NOT NULL,
  retention_micros INTEGER,
  enabled INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS _tts_hypertable_stats (
  hypertable_id INTEGER PRIMARY KEY,
  row_count INTEGER NOT NULL,
  min_time_micros INTEGER,
  max_time_micros INTEGER,
  chunk_count INTEGER NOT NULL,
  series_count INTEGER NOT NULL,
  updated_at_micros INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS _tts_series_stats (
  series_id INTEGER PRIMARY KEY,
  hypertable_id INTEGER NOT NULL,
  min_time_micros INTEGER,
  max_time_micros INTEGER,
  row_count INTEGER NOT NULL,
  chunk_count INTEGER NOT NULL,
  updated_at_micros INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS _tts_maintenance_jobs (
  id INTEGER PRIMARY KEY,
  job_kind TEXT NOT NULL, -- compact | downsample | retention | stats
  target_id INTEGER,
  due_at_micros INTEGER NOT NULL,
  locked_at_micros INTEGER,
  completed_at_micros INTEGER,
  error TEXT
);
```

---

## 10. Segment format

### 10.1 Segment goals

The segment format is TSM-inspired, but stored inside Turso BLOBs.

Goals:

- fast append batches
- fast time pruning
- per-column decoding
- dictionary-encoded tags
- min/max statistics
- segment-level checksums
- forward-compatible sections

### 10.2 Segment layout

```text
TTS_SEGMENT_V1
  header
    magic
    version
    flags
    schema_hash
    chunk_id
    min_time
    max_time
    row_count

  series_dictionary
    series_id
    tag_key/value dictionary ids

  column_directory
    column_id
    logical_type
    encoding
    offset
    length
    null_bitmap_offset
    min_value
    max_value

  encoded_blocks
    time column
    tag columns
    field columns

  footer
    index_offset
    checksum
```

### 10.3 Encoding choices

| Data type | Initial encoding |
|---|---|
| time | delta + varint |
| integer | delta + varint |
| float | XOR/Gorilla-style |
| text/tag | dictionary encoding |
| bool | bitpack/RLE |
| nulls | bitmap |
| blob | length-prefixed raw initially |

---

## 11. WASM extension path

### 11.1 Target

Primary extension target:

```bash
wasm32-unknown-unknown
```

Not primary:

```text
wasm32-wasip1
wasm32-wasi
wasm32-wasix
wasm-bindgen browser target
```

### 11.2 WASM extension crate

```toml
[package]
name = "turso-timeseries-wasm-ext"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
turso-timeseries-core = { path = "../turso-timeseries-core", default-features = false, features = ["alloc", "line-protocol"] }
turso-wasm-sdk = { git = "https://github.com/tursodatabase/turso", branch = "turso-wasm-udf", package = "turso-wasm-sdk" }

[features]
default = []
```

### 11.3 Initial scalar exports

```rust
#![no_std]

extern crate alloc;

use alloc::string::String;
use turso_wasm_sdk::turso_wasm;

#[turso_wasm]
fn tts_version() -> String {
    "turso-timeseries 0.1.0".into()
}

#[turso_wasm]
fn time_bucket(width: &str, ts_micros: i64) -> i64 {
    match turso_timeseries_core::query::time_bucket::time_bucket(width, ts_micros) {
        Ok(v) => v,
        Err(_) => 0,
    }
}

#[turso_wasm]
fn tts_parse_duration_micros(width: &str) -> Option<i64> {
    turso_timeseries_core::model::time::parse_duration_micros(width).ok()
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}
```

### 11.4 Extension manifest

Initial manifest:

```json
{
  "functions": [
    { "name": "tts_version", "export": "tts_version", "narg": 0 },
    { "name": "time_bucket", "export": "time_bucket", "narg": 2 },
    { "name": "tts_parse_duration_micros", "export": "tts_parse_duration_micros", "narg": 1 }
  ]
}
```

Later manifest:

```json
{
  "functions": [
    { "name": "tts_version", "export": "tts_version", "narg": 0 },
    { "name": "time_bucket", "export": "time_bucket", "narg": 2 },
    { "name": "tts_refresh_rollup", "export": "tts_refresh_rollup", "narg": 1 }
  ],
  "vtabs": [
    {
      "name": "tts_scan",
      "columns": [
        ["hypertable", "TEXT HIDDEN"],
        ["time", "INTEGER"],
        ["series_id", "INTEGER"],
        ["value", "REAL"]
      ],
      "open": "tts_scan_open",
      "filter": "tts_scan_filter",
      "column": "tts_scan_column",
      "next": "tts_scan_next",
      "eof": "tts_scan_eof",
      "rowid": "tts_scan_rowid"
    }
  ]
}
```

---

## 12. Native Rust path

### 12.1 Native crate API

```rust
pub struct Timeseries;

impl Timeseries {
    pub async fn install(conn: &turso::Connection) -> Result<()>;

    pub async fn create_hypertable(
        conn: &turso::Connection,
        table: &str,
        time_column: &str,
    ) -> Result<HypertableId>;

    pub async fn write_line_protocol(
        conn: &turso::Connection,
        line: &str,
    ) -> Result<WriteStats>;

    pub async fn write_points(
        conn: &turso::Connection,
        table: &str,
        points: &[MetricPoint],
    ) -> Result<WriteStats>;

    pub async fn run_maintenance(
        conn: &turso::Connection,
        options: MaintenanceOptions,
    ) -> Result<MaintenanceReport>;
}
```

### 12.2 Native example

```rust
use turso_timeseries::Timeseries;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let db = turso::Builder::new_local("metrics.db").build().await?;
    let conn = db.connect().await?;

    Timeseries::install(&conn).await?;

    Timeseries::create_hypertable(
        &conn,
        "runtime_metrics",
        "time",
    ).await?;

    Timeseries::write_line_protocol(
        &conn,
        "runtime_metrics,task=MainTask,variable=Motor.Speed value=1420 1778000000000000000",
    ).await?;

    Timeseries::run_maintenance(
        &conn,
        MaintenanceOptions {
            compact: true,
            downsample: true,
            retention: true,
            budget_ms: Some(50),
        },
    ).await?;

    Ok(())
}
```

---

## 13. Browser loading path

### 13.1 Raw browser example

```ts
import { connect } from "@tursodatabase/database-wasm";

async function wasmToHex(url: string): Promise<string> {
  const bytes = new Uint8Array(await fetch(url).then(r => r.arrayBuffer()));
  return [...bytes].map(b => b.toString(16).padStart(2, "0")).join("");
}

const db = await connect("metrics.db");

const hex = await wasmToHex("/turso_timeseries_ext.wasm");

await db.exec(`
  CREATE EXTENSION turso_timeseries
  LANGUAGE wasm
  AS X'${hex}'
`);

const row = await db.prepare(`
  SELECT time_bucket('5m', 1778000123456789) AS bucket
`).get();

console.log(row);
```

### 13.2 JS helper package

```ts
import { connect } from "@tursodatabase/database-wasm";
import { loadTimeseriesExtension } from "@hyperdrive-technology/turso-timeseries";

const db = await connect("metrics.db");

await loadTimeseriesExtension(db, {
  wasmUrl: "/turso_timeseries_ext.wasm",
});

await db.exec(`
  SELECT tts_version()
`);
```

---

## 14. Aggregation strategy without aggregate UDF support

### 14.1 Principle

Do **not** block on generic SQL aggregate/window UDF registration.

Implement aggregates as internal TSDB operators:

```rust
pub trait AggregateState {
    fn push(&mut self, value: FieldValue, time: TimestampMicros);
    fn finish(&self) -> FieldValue;
}

pub struct AvgState {
    sum: f64,
    count: u64,
}

impl AggregateState for AvgState {
    fn push(&mut self, value: FieldValue, _time: TimestampMicros) {
        if let FieldValue::F64(v) = value {
            self.sum += v;
            self.count += 1;
        }
    }

    fn finish(&self) -> FieldValue {
        if self.count == 0 {
            FieldValue::Null
        } else {
            FieldValue::F64(self.sum / self.count as f64)
        }
    }
}
```

### 14.2 Initial aggregate list

P0:

- count
- sum
- avg
- min
- max
- first
- last

P1:

- delta
- rate
- increase
- stddev
- variance

P2:

- approximate percentile
- histogram
- median approximation
- counter-reset-aware rate/increase

### 14.3 Query API

```sql
SELECT *
FROM tts_aggregate
WHERE hypertable = 'runtime_metrics'
  AND bucket = '5m'
  AND field = 'value'
  AND time >= 1778000000000000
  AND time < 1778086400000000;
```

Alternative explicit API:

```sql
SELECT *
FROM tts_query(
  '{
    "hypertable": "runtime_metrics",
    "from": 1778000000000000,
    "to": 1778086400000000,
    "bucket": "5m",
    "group_by": ["task", "variable"],
    "aggregates": {
      "value": ["avg", "min", "max", "count"]
    }
  }'
);
```

---

## 15. Window-like functions

### 15.1 Principle

Implement TSDB windows internally, not through SQL `OVER (...)` in v1.

### 15.2 Window operators

P1:

- moving average
- rolling min
- rolling max
- rolling sum
- derivative
- elapsed

P2:

- gap fill
- locf
- interpolation
- state duration
- counter reset detection

### 15.3 Internal state

```rust
pub struct MovingAvg {
    window_micros: i64,
    points: alloc::collections::VecDeque<(TimestampMicros, f64)>,
    sum: f64,
}

impl MovingAvg {
    pub fn push(&mut self, ts: TimestampMicros, value: f64) -> Option<f64> {
        self.points.push_back((ts, value));
        self.sum += value;

        while let Some((old_ts, old_value)) = self.points.front().copied() {
            if ts.0 - old_ts.0 > self.window_micros {
                self.points.pop_front();
                self.sum -= old_value;
            } else {
                break;
            }
        }

        if self.points.is_empty() {
            None
        } else {
            Some(self.sum / self.points.len() as f64)
        }
    }
}
```

### 15.4 Query API

```sql
SELECT *
FROM tts_window(
  '{
    "hypertable": "runtime_metrics",
    "from": 1778000000000000,
    "to": 1778086400000000,
    "partition_by": ["task", "variable"],
    "order_by": "time",
    "window": "5m",
    "step": "1m",
    "functions": {
      "value": ["moving_avg", "rolling_min", "rolling_max"]
    }
  }'
);
```

---

## 16. Downsampling and rollups

### 16.1 Rollup model

```text
raw hypertable:
  runtime_metrics
  retention: 7 days

rollup hypertables:
  runtime_metrics_1m
  retention: 90 days

  runtime_metrics_5m
  retention: 1 year

  runtime_metrics_1h
  retention: 5 years
```

### 16.2 Rollup policy API

```sql
SELECT tts_create_rollup_policy(
  'runtime_metrics',
  'runtime_metrics_5m',
  '5m',
  '{
    "value": ["avg", "min", "max", "count"]
  }'
);
```

### 16.3 Invalidations

Every write marks affected rollup ranges dirty:

```rust
pub fn invalidate_for_write(
    hypertable: HypertableId,
    points: &[MetricPoint],
    catalog: &mut dyn CatalogStore,
) -> Result<()> {
    let min = points.iter().map(|p| p.time).min().unwrap();
    let max = points.iter().map(|p| p.time).max().unwrap();

    catalog.mark_invalidated(
        hypertable,
        TimeRange { start: min, end: max },
        InvalidationReason::Write,
    )
}
```

### 16.4 Refresh strategy

```text
refresh_rollup(policy):
  find invalidations
  expand to bucket boundaries
  read raw chunks for affected buckets
  recompute aggregate states
  replace affected rollup segment(s)
  mark invalidations processed
```

---

## 17. Join strategy with regular Turso tables

### 17.1 Goal

Support queries like:

```sql
SELECT
  d.site,
  d.asset_name,
  m.bucket,
  m.avg_value
FROM devices d
JOIN tts_rollup_5m m
  ON m.device_id = d.device_id
WHERE d.site = 'Brisbane'
  AND m.bucket >= 1778000000000000
  AND m.bucket < 1778086400000000;
```

### 17.2 Constraint-aware vtabs

Expose virtual tables that can accept constraints:

- `hypertable = ?`
- `time >= ?`
- `time < ?`
- `bucket >= ?`
- `bucket < ?`
- `device_id = ?`
- `series_id = ?`
- `field = ?`

### 17.3 Recommended vtabs

| Vtab | Purpose | Main use |
|---|---|---|
| `tts_scan` | Raw point scan | Debug/narrow time ranges |
| `tts_aggregate` | On-demand bucketed aggregate | Ad hoc queries |
| `tts_rollup` | Materialized rollup scan | Dashboards and joins |
| `tts_last_value` | Latest values | Status panels |
| `tts_series` | Series/tag metadata | Joinable metadata |
| `tts_chunks` | Chunk metadata | Admin/debug |

### 17.4 Cost model

The vtab must provide estimates based on:

- hypertable row count
- time range selectivity
- series/tag selectivity
- chunk count
- rollup resolution
- whether constraints include equality on series/tag key

Pseudo-estimator:

```rust
pub fn estimate_scan_cost(
    stats: &HypertableStats,
    constraints: &ScanConstraints,
) -> ScanEstimate {
    let time_fraction = constraints
        .time_range
        .map(|r| r.width() as f64 / stats.total_time_width() as f64)
        .unwrap_or(1.0)
        .clamp(0.0, 1.0);

    let series_fraction = constraints
        .series_eq_count
        .map(|n| n as f64 / stats.series_count.max(1) as f64)
        .unwrap_or(1.0)
        .clamp(0.0, 1.0);

    let estimated_rows = (stats.row_count as f64 * time_fraction * series_fraction)
        .max(1.0) as u64;

    ScanEstimate {
        estimated_rows,
        estimated_cost: estimated_rows as f64,
    }
}
```

### 17.5 Explicit query escape hatch

For complex joins where planner pushdown is not enough:

```sql
SELECT device_id
FROM devices
WHERE site = 'Brisbane';
```

Then pass selected IDs into `tts_query`:

```sql
SELECT *
FROM tts_query(
  '{
    "hypertable": "runtime_metrics",
    "from": 1778000000000000,
    "to": 1778086400000000,
    "bucket": "5m",
    "filters": {
      "device_id": ["D1", "D2", "D3"]
    },
    "aggregates": {
      "value": ["avg", "min", "max"]
    }
  }'
);
```

---

## 18. MVCC, transaction, and visibility semantics

### 18.1 Rule

A point batch becomes visible atomically only after:

1. segment BLOB is inserted
2. chunk metadata is updated
3. series/tag indexes are updated
4. stats are updated
5. invalidation rows are inserted
6. transaction commits

### 18.2 Write flow

```text
parse input
validate hypertable schema
map series/tag ids
group points by chunk
encode segment(s)
BEGIN
  insert/update series rows
  insert segment rows
  update chunk row counts/minmax
  update stats
  insert invalidation rows
COMMIT
```

### 18.3 Rollback test

Test must verify:

```text
BEGIN
  write segment
  update chunk
ROLLBACK

query:
  no points visible
  no chunk stats visible
  no invalidation visible
```

---

## 19. OPFS/browser compatibility

### 19.1 Principle

`turso-timeseries` does not access OPFS directly.

Browser persistence is owned by Turso’s browser database runtime.

`turso-timeseries` stores TSDB state in ordinary Turso tables/BLOBs.

### 19.2 Avoid

- `std::fs`
- WASI filesystem
- WASIX filesystem
- direct OPFS JS APIs in core
- sidecar segment files
- browser global `fetch` in extension core

### 19.3 Allow

- JS helper may `fetch()` the `.wasm` artifact to load extension bytes.
- Turso browser package owns OPFS.
- Turso browser package owns database persistence.
- Future sync package owns sync push/pull/checkpoint.

---

## 20. Testing plan

### 20.1 Core tests

```text
codec_roundtrip.rs
  timestamp delta encoding
  integer delta encoding
  float XOR encoding
  dictionary tags
  null bitmaps
  segment checksum

line_protocol.rs
  measurement/tags/fields/timestamp
  escaping
  malformed lines
  batch partial failure

time_bucket.rs
  second/minute/hour buckets
  boundary conditions
  negative/invalid durations

aggregate_states.rs
  count/sum/avg/min/max
  null handling
  first/last ordering

window_states.rs
  moving average
  rolling min/max
  fixed window eviction

downsample.rs
  raw -> 1m
  raw -> 5m
  late data invalidates dirty buckets

compaction.rs
  query equivalence before/after compaction
  duplicate point resolution
```

### 20.2 Native integration tests

```text
install.rs
  idempotent install
  migration versioning

write_read.rs
  create hypertable
  write line protocol
  read raw points
  read aggregate rows

transaction_visibility.rs
  rollback hides segment
  commit reveals segment

vtab_join.rs
  devices JOIN tts_rollup
  time constraint pushdown
  tag equality pushdown
  row estimate sanity

maintenance.rs
  compaction
  rollup refresh
  retention
```

### 20.3 WASM extension tests

```text
wasm_build.rs
  cargo build --target wasm32-unknown-unknown

wasm_exports.rs
  wasm exports memory
  wasm exports turso_malloc
  wasm exports tts_version
  wasm exports time_bucket
  wasm exports turso_ext_init when manifest is enabled

browser_create_extension.spec.ts
  connect database-wasm
  CREATE EXTENSION turso_timeseries
  SELECT tts_version()
  SELECT time_bucket(...)
```

### 20.4 Browser tests

```text
browser-local.spec.ts
  open database-wasm
  load extension
  create hypertable
  write batch
  query rollup
  reload page
  verify persistence

browser-vtab.spec.ts
  load extension
  create devices table
  insert devices
  query join with tts_rollup
```

### 20.5 Benchmarks

```text
ingest.rs
  1k / 10k / 100k points
  low cardinality / high cardinality

query.rs
  raw scan
  aggregate scan
  rollup scan
  last value query

compaction.rs
  segment rewrite throughput
  compression ratio

downsample.rs
  dirty bucket refresh
  full refresh
```

---

## 21. Implementation phases

### Phase 0 — Repo cleanup and README reset

Goal: make the repo standalone and remove references to consuming apps such as Hyperdrive/IEC VPLC.

Tasks:

- Add `PLAN-v2.md`.
- Update README to describe `turso-timeseries` as standalone.
- Remove consuming app references.
- Add “Extension targets” section:
  - native Rust crate
  - Turso WASM extension
  - optional JS helper
- Add “Not WASI/WASIX” clarification.
- Add current unstable dependency note for Turso PR #6256-style WASM extension ABI.

Acceptance:

- README can be read without knowing Hyperdrive.
- No required reference to IEC runtime.
- `PLAN-v2.md` documents all intended paths.

---

### Phase 1 — Core crate foundation

Goal: establish portable core model and codecs.

Tasks:

- Create/clean `turso-timeseries-core`.
- Add:
  - `MetricPoint`
  - `FieldValue`
  - `Hypertable`
  - `ChunkMeta`
  - `Segment`
  - duration parser
  - `time_bucket`
  - aggregate states
- Implement basic line protocol parser.
- Implement segment format v0.
- Implement roundtrip tests.

Code target:

```rust
let bucket = time_bucket("5m", TimestampMicros(1778000123456789))?;
assert_eq!(bucket, TimestampMicros(1778000100000000));
```

Acceptance:

- `cargo test -p turso-timeseries-core`
- `cargo test -p turso-timeseries-core --no-default-features` if practical
- no Turso dependency in core

---

### Phase 2 — Catalog and native install

Goal: install schema into Turso/native connection.

Tasks:

- Add `turso-timeseries-catalog`.
- Add migrations.
- Add schema versioning.
- Add idempotent install.
- Add native `Timeseries::install`.

Code target:

```rust
let db = turso::Builder::new_local(":memory:").build().await?;
let conn = db.connect().await?;
Timeseries::install(&conn).await?;
```

Acceptance:

- catalog tables exist
- install is idempotent
- migration version recorded

---

### Phase 3 — Native write/read path

Goal: write line protocol to Turso BLOB segments and query it back.

Tasks:

- `create_hypertable`
- `write_line_protocol`
- `write_points`
- chunk selection
- series/tag indexing
- segment BLOB insert
- basic raw scan

Acceptance:

- write points
- read points
- rollback hides writes
- commit reveals writes

---

### Phase 4 — WASM scalar extension proof

Goal: build raw Turso WASM extension following PR #6256 example.

Tasks:

- Create `turso-timeseries-wasm-ext`.
- Use `#![no_std]`.
- Use `extern crate alloc`.
- Use `turso-wasm-sdk`.
- Export:
  - `tts_version`
  - `time_bucket`
  - `tts_parse_duration_micros`
- Build with `wasm32-unknown-unknown`.

Commands:

```bash
rustup target add wasm32-unknown-unknown

cargo build \
  -p turso-timeseries-wasm-ext \
  --target wasm32-unknown-unknown \
  --release \
  --no-default-features
```

Acceptance:

- artifact builds
- exports expected symbols
- browser can `CREATE FUNCTION` or `CREATE EXTENSION`
- `SELECT tts_version()` works
- `SELECT time_bucket('5m', ?)` works

---

### Phase 5 — WASM extension manifest

Goal: support `CREATE EXTENSION turso_timeseries LANGUAGE wasm AS X'...'`.

Tasks:

- Implement `turso_ext_init`.
- Return JSON manifest.
- Declare scalar functions.
- Add manifest validation test.

Acceptance:

- one `CREATE EXTENSION` registers all scalar functions.
- no separate `CREATE FUNCTION` required.

---

### Phase 6 — Internal aggregate engine

Goal: aggregate state machines independent of SQL aggregate UDF support.

Tasks:

- Implement aggregate states:
  - count
  - sum
  - avg
  - min
  - max
  - first
  - last
- Add bucketed aggregation executor.
- Add tests comparing raw aggregate results.

Acceptance:

- raw scan and aggregate query produce expected rows.
- no dependency on generic SQL aggregate UDFs.

---

### Phase 7 — Rollups and downsampling

Goal: materialized rollup hypertables.

Tasks:

- Add rollup policy catalog.
- Add invalidation tracking.
- Add `tts_create_rollup_policy`.
- Add `tts_refresh_rollup`.
- Add rollup segment encoding.
- Add retention policy support.

Acceptance:

- raw writes create invalidations.
- refresh recomputes dirty buckets.
- rollup query equals raw aggregate query.
- late data refreshes only affected buckets.

---

### Phase 8 — Virtual table MVP

Goal: expose queryable TSDB surfaces as vtabs.

Tasks:

- Implement vtab manifest entries.
- Implement:
  - `tts_chunks`
  - `tts_series`
  - `tts_last_value`
- Then implement:
  - `tts_scan`
  - `tts_aggregate`
  - `tts_rollup`
- Support hidden columns for scan parameters.

Initial vtab columns:

```text
tts_rollup:
  hypertable HIDDEN
  bucket_width HIDDEN
  field HIDDEN
  time INTEGER
  series_id INTEGER
  device_id TEXT
  avg_value REAL
  min_value REAL
  max_value REAL
  count_value INTEGER
```

Acceptance:

```sql
SELECT *
FROM tts_rollup
WHERE hypertable = 'runtime_metrics'
  AND bucket_width = '5m'
  AND time >= ?
  AND time < ?;
```

---

### Phase 9 — Join pushdown and stats

Goal: make joins with regular Turso tables practical.

Tasks:

- Add hypertable stats.
- Add series stats.
- Add tag index.
- Implement cost/row estimates.
- Push down:
  - time range
  - series equality
  - tag equality
  - field equality
- Add join tests.

Acceptance:

```sql
SELECT d.asset_name, m.time, m.avg_value
FROM devices d
JOIN tts_rollup m ON m.device_id = d.device_id
WHERE d.site = 'Brisbane'
  AND m.hypertable = 'runtime_metrics'
  AND m.time >= ?
  AND m.time < ?;
```

Should avoid full TSDB scan when possible.

---

### Phase 10 — Maintenance engine

Goal: compaction, retention, stats refresh, and rollup refresh.

Tasks:

- `run_maintenance`
- compaction jobs
- retention jobs
- stats refresh jobs
- rollup refresh jobs
- budgeted execution

API:

```rust
Timeseries::run_maintenance(
    &conn,
    MaintenanceOptions {
        compact: true,
        downsample: true,
        retention: true,
        refresh_stats: true,
        budget_ms: Some(50),
    },
).await?;
```

Acceptance:

- idempotent
- resumable
- budget-respecting
- safe across transactions

---

### Phase 11 — JS helper package

Goal: browser ergonomics without confusing it with the extension ABI.

Tasks:

- Add `packages/turso-timeseries-js`.
- Add `loadTimeseriesExtension(db, { wasmUrl })`.
- Add helpers:
  - `createHypertable`
  - `writeLineProtocol`
  - `queryRollup`
  - `runMaintenance`

Acceptance:

```ts
await loadTimeseriesExtension(db, { wasmUrl: "/turso_timeseries_ext.wasm" });
await db.exec("SELECT tts_version()");
```

---

### Phase 12 — Optional Arrow/DataFusion adapters

Goal: native/server acceleration and interoperability.

Tasks:

- Add Arrow export for scans/rollups.
- Add optional DataFusion `TableProvider`.
- Keep out of browser default bundle.

Acceptance:

- feature-gated
- native-only by default
- no impact on WASM extension build

---

### Phase 13 — Future sync repo

Goal: sync-aware orchestration outside this repo.

Future repo:

```text
hyperdrive-technology/turso-timeseries-sync
```

Responsibilities:

- `push/pull/checkpoint` orchestration
- sync-aware compaction barriers
- post-pull invalidation refresh
- multi-device conflict handling
- compaction churn policy

This repo should expose enough metadata/hooks for sync, but not own sync itself.

---

## 22. README replacement outline

Recommended README structure:

```md
# turso-timeseries

A pure-Rust time-series extension for Turso.

`turso-timeseries` provides TimescaleDB-inspired hypertables and rollups,
InfluxDB-inspired line protocol ingest and TSM-style segment storage,
and a Turso-native WASM extension artifact for browser/local-first use.

## Status

Experimental. The browser-native extension path follows Turso's unstable
WASM extension design from PR #6256.

## Targets

- Native Rust integration with the `turso` crate
- Browser-native Turso WASM extension via `CREATE EXTENSION ... LANGUAGE wasm`
- Optional JavaScript helper package

## Not WASI/WASIX

The browser extension is not a WASI or WASIX application. It is a raw
WebAssembly module that exports the symbols expected by Turso's WASM
extension ABI.

## Features

- hypertables
- chunks
- TSM-style segment encoding
- line protocol ingest
- time_bucket
- rollups/downsampling
- retention
- constraint-aware virtual tables
- joins with regular Turso tables

## Quick start

...
```

---

## 23. Open technical risks

| Risk | Impact | Mitigation |
|---|---|---|
| PR #6256 closed/unmerged | WASM extension ABI may change or not land | Keep `wasm-ext` behind unstable feature; isolate ABI crate |
| WASM vtab support incomplete | joins and table scans delayed | start with scalar functions + native path + explicit query API |
| No aggregate/window UDF support | cannot expose generic `tts_avg()` SQL aggregate | implement aggregates internally via vtabs/query/rollups |
| Cost estimates too weak | bad join plans | maintain stats tables and provide explicit `tts_query` escape hatch |
| BLOB segment churn hurts sync | future sync inefficiency | defer sync repo; add compaction barriers/generation tracking later |
| Browser memory limits | large scans fail | require time range constraints, prefer rollups, stream rows |
| DataFusion too heavy for browser | bundle/perf issues | optional native-only adapter |
| WASM SDK instability | breakage | pin branch/commit until upstream stable |
| Full Timescale parity too broad | scope explosion | tiered features, document subset |

---

## 24. Final intended architecture

```text
                         regular Turso tables
                       devices / assets / config
                                  │
                                  │ joins
                                  ▼
┌──────────────────────────────────────────────────────────┐
│                  Turso SQL planner/runtime               │
├──────────────────────────────────────────────────────────┤
│ scalar functions                                          │
│   tts_version()                                           │
│   time_bucket()                                           │
│   parse_duration()                                        │
│                                                          │
│ virtual tables                                            │
│   tts_scan                                                │
│   tts_aggregate                                           │
│   tts_rollup                                              │
│   tts_last_value                                          │
│   tts_series                                              │
│   tts_chunks                                              │
├──────────────────────────────────────────────────────────┤
│              turso-timeseries extension layer             │
│  query planning / constraint pushdown / aggregate engine  │
├──────────────────────────────────────────────────────────┤
│                  Turso catalog/BLOB storage               │
│  _tts_hypertables / _tts_chunks / _tts_segments / stats   │
├──────────────────────────────────────────────────────────┤
│                  Turso MVCC / OPFS / native storage       │
└──────────────────────────────────────────────────────────┘
```

---

## 25. Final decision log

1. Use `hyperdrive-technology/turso-timeseries` as a focused repo.
2. Build two primary releases:
   - Rust/native crate
   - browser-native WASM extension artifact
3. Do not vendor Turso.
4. Do not target WASIX.
5. Do not use `wasm-bindgen` for the extension ABI.
6. Follow Turso PR #6256 extension model.
7. Store TSDB segments inside Turso BLOBs for v1.
8. Implement aggregate/window semantics internally.
9. Use vtabs for queryable hypertables and joins.
10. Maintain stats for cost estimation and join planning.
11. Keep sync in a later separate repo.
12. Keep Arrow/DataFusion optional and native/server-first.
