# turso-timeseries — master plan

**Repository:** [github.com/hyperdrive/turso-timeseries](https://github.com/hyperdrive/turso-timeseries) (publish target)  
**Status:** scaffold + README; this document is the canonical roadmap until superseded.  
**Last reviewed:** 2026-05-12  
**Reference posture:** Turso-native implementation; TimescaleDB and InfluxDB are design references, not compatibility targets.

---

## 1. Purpose

Deliver a **Rust-first, Turso-native time-series layer** for Hyperdrive that can run across:

- **native Rust / embedded Linux** by linking the `turso` crate directly;
- **browser/local-first WASM** by using Turso's browser packages (`@tursodatabase/database-wasm` and `@tursodatabase/sync-wasm`) and compiling Hyperdrive runtime pieces to WASM;
- **real-time-ish Linux deployments** by keeping IEC scan work out of database write/maintenance paths;
- **Embassy / MCU deployments** by running only the IEC control/runtime subset and sending telemetry to a gateway/host that runs Turso.

The goal is not merely helper SQL. The longer-term goal is a **portable TSDB layer beside Turso** with:

- TimescaleDB-inspired ergonomics: hypertable-like catalog, bucket functions, retention, compression/downsampling policies, continuous-aggregate-style rollups.
- InfluxDB-inspired ingest/storage: fast streaming ingest, line-protocol-compatible ingest path where useful, TSM-like append/compact storage ideas, last-value/tag-cache concepts.
- Turso-native persistence/sync: catalog and chunk state stored inside Turso tables/BLOBs first, so sync/checkpoint/MVCC semantics remain Turso-owned.

This library should not pretend to be PostgreSQL or InfluxDB. It should expose a clean Turso/SQLite-compatible API that fits Hyperdrive's IEC 61131 runtime and historian/trend use cases.

---

## 2. Current Turso facts this plan depends on

These upstream behaviours materially affect the architecture:

| Fact | Planning implication |
|------|----------------------|
| Turso Database is an embedded/in-process SQLite-compatible Rust database and is positioned to run offline, on-device, and in the browser. | Treat Turso as the durable local DB boundary for native, browser, and embedded-Linux targets. |
| For new SDK usage, Turso distinguishes local embedded database, local database + sync, remote/serverless access, and legacy libSQL. | Do not design `turso-timeseries` around `@tursodatabase/serverless`; browser/local-first work should use database/sync packages. |
| Browser Turso uses `@tursodatabase/database-wasm`; it is async because OPFS is async, has bundler-specific exports, and uses WASM + workers/SharedArrayBuffer in its implementation. | Do not assume a single monolithic WASM artifact early. Compose Turso WASM with Hyperdrive WASM modules. |
| Browser sync uses `@tursodatabase/sync-wasm`; sync exposes explicit `push`, `pull`, and `checkpoint` concepts. | Keep TSDB catalog/chunks inside the DB file first, so sync owns transport and reconciliation. |
| Turso supports Turso-native extensions via `load_extension()`; SQLite `.so`/`.dll` loadable extensions are not supported. | Extension work must target Turso's Rust extension system, not classic SQLite extension ABI assumptions. |
| Turso has a `turso_ext` crate with macros and virtual table/function extension API surface. | Investigate implementing SQL functions/table functions as a Turso-native extension, with static linking for WASM/browser if dynamic loading is not viable. See `docs/TURSO_EXTENSION_API.md` (factual summary of upstream `extensions/core/README.md`). |
| Turso has built-in time, percentile, UUID, vector, FTS, CSV, regexp and `generate_series` extension functionality. | Avoid reinventing generic time/percentile primitives where built-ins are available; focus on TSDB-specific storage, rollups, and planner/query helpers. |
| Turso has Turso-specific features such as `BEGIN CONCURRENT`, MVCC/concurrent write direction, CDC, materialized views, custom index methods, and richer types. | Explore MVCC-safe ingest and future use of materialized views/CDC/custom indexes, but do not make experimental features mandatory in v1. |

---

## 3. Product definition

`turso-timeseries` is:

> A pure-Rust, portable time-series layer for Turso. It provides TimescaleDB-inspired hypertable ergonomics, time buckets, retention/downsampling policies and rollup tables; InfluxDB-inspired streaming ingest, TSM-style storage concepts and optional line protocol ingest; and Turso-native catalog/chunk persistence that can work with native Turso, browser WASM Turso, and local-first sync.

Short version:

```text
Turso persistence + Timescale ergonomics + Influx ingest/storage ideas
```

---

## 4. Goals

| Goal | Detail |
|------|--------|
| **One TSDB core** | Shared Rust code for time representation, series identity, bucket math, aggregation windows, chunk metadata, invalidation tracking, rollup planning, and codec abstractions. |
| **Turso-native SQL surface** | SQL functions/table functions where Turso extension APIs support them; fallback Rust/JS wrappers where browser extension registration is not yet stable. |
| **Fast ingest path** | Batch/streaming ingest with write buffering, compact internal representation, and a clear path to TSM-like sealed segments. |
| **Downsampling as a first-class feature** | Materialized rollup tables/chunks, dirty-bucket invalidation, retention integration, and multi-resolution query routing over time. |
| **Native and browser runtime support** | Native Rust links `turso` as a crate; browser uses `@tursodatabase/database-wasm` + `@tursodatabase/sync-wasm` and a WASM wrapper around shared TSDB core. |
| **Embedded Linux support** | Single Rust binary for Linux gateways/soft PLC devices, with IEC runtime, Turso, and TSDB linked together. |
| **Real-time isolation** | PREEMPT_RT profile keeps IEC scan loop isolated from database writes, compaction, rollups and sync. |
| **Embassy compatibility at runtime-core level** | IEC runtime core may be `no_std`/Embassy-capable; Turso/TSDB persistence does not run on MCU firmware. |
| **Sync-safe persistence** | Store catalog and v1 segments inside Turso tables/BLOBs so Turso sync/checkpoint can own transport and durability. |

---

## 5. Non-goals

- Full TimescaleDB SQL or PostgreSQL compatibility.
- Full InfluxDB, InfluxQL, Flux, or InfluxDB 3 server compatibility.
- Embedding InfluxDB as a library.
- Running Turso or the full TSDB engine on Embassy/bare-metal MCU targets.
- Classic SQLite `.so` / `.dll` extension compatibility.
- Designing a new sync protocol outside Turso Sync.
- DuckDB-scale OLAP inside Turso v1.
- Hard real-time database writes inside an IEC scan cycle.

---

## 6. Target matrix

| Target | Packaging recommendation | Turso approach | `turso-timeseries` approach | IEC 61131 runtime approach | Notes |
|---|---|---|---|---|---|
| **Browser / local-first web app** | Multiple WASM/JS modules, likely orchestrated by workers | `@tursodatabase/database-wasm` + `@tursodatabase/sync-wasm` | Compile core/wrapper to WASM; integrate beside Turso WASM | Compile runtime to WASM | Best for IDE, simulator, dashboard, debugging, browser-only operation. |
| **Native desktop / server Rust app** | Single Rust binary | `turso` crate | Link as Rust crates | Link as Rust crates | Cleanest development and backend runtime path. |
| **Embedded Linux gateway** | Single Rust binary | `turso` crate | Link as Rust crates | Link as Rust crates | Best full offline-first runtime + historian + sync target. |
| **Embedded Linux + PREEMPT_RT** | Single binary with isolated priority domains | `turso` crate outside RT loop | Non-RT writer/maintenance threads | RT scan loop; no DB writes inside scan loop | Use bounded queues from IEC loop to TSDB writer. |
| **Embassy / bare-metal MCU** | Firmware image, no Turso | Do not run Turso | Do not run full TSDB; emit telemetry | `no_std` runtime core on Embassy | Pair with gateway/browser/native host. |
| **MCU + Linux gateway** | MCU firmware + gateway binary | Turso on gateway | TSDB on gateway | IEC control on MCU; gateway receives telemetry | Strong industrial deployment pattern. |
| **Remote/serverless-only app** | Client + remote Turso | `@tursodatabase/serverless` only if remote has extension | Requires server-side extension support | Usually not local | Not the main Hyperdrive historian path. |
| **Self-hosted Turso server** | Custom server/build | Link/enable extension server-side | Native extension/server integration | Optional | Useful later for central query/sync nodes. |
| **Mobile app** | Native/mobile package or WASM-like package | Platform-dependent Turso SDK | Shared core with mobile adapter | Shared runtime core | Treat like embedded/desktop depending on API surface. |

---

## 7. Architecture overview

```text
IEC 61131 runtime / app host
        |
        v
Metric/Event sink trait
        |
        +-----------------------------+
        |                             |
        v                             v
RT-safe bounded queue          direct batch API
        |                             |
        +-------------+---------------+
                      |
                      v
              turso-timeseries
   +---------------------------------------+
   | catalog + schema                      |
   | ingest buffer / WAL tables            |
   | series dictionary                     |
   | TSM-like segment codec                |
   | chunk metadata + stats                |
   | dirty-bucket invalidations            |
   | rollup/downsampling engine            |
   | retention/maintenance engine          |
   | SQL functions/table functions         |
   +---------------------------------------+
                      |
                      v
                  Turso DB
   +---------------------------------------+
   | _tts_* catalog tables                 |
   | raw sample tables or segment BLOBs    |
   | rollup tables / rollup segment BLOBs  |
   | metadata, checkpoints, jobs           |
   +---------------------------------------+
                      |
                      v
          Turso Sync / local file / OPFS
```

---

## 8. Project skeleton

Initial repo shape should support both current minimal implementation and the longer TSDB path without splitting too early.

```text
turso-timeseries/
  Cargo.toml
  rust-toolchain.toml              # introduced once Turso integration is pinned
  README.md
  PLAN.md

  crates/
    turso-timeseries/
      # v0/v1 default crate: pure-ish core, bucket math, SQL templates,
      # catalog model, migrations, simple row-table layout helpers.

    turso-timeseries-core/
      # later split if needed:
      # time, bucket, series, rollup, retention, invalidation,
      # chunk metadata, codec traits, no direct Turso dependency.

    turso-timeseries-codec/
      # TSM-like encoding/decoding, block format, checksums,
      # delta/delta-of-delta, dictionary encoding, optional compression.

    turso-timeseries-catalog/
      # migration model, _tts_* schema, schema versions,
      # table/column definitions, job metadata.

    turso-timeseries-influx/
      # optional line protocol parser/adapter and Influx-style
      # measurement/tag/field mapping.

    turso-timeseries-turso/
      # native integration with the Rust `turso` crate:
      # install(), migrations, batch writer, maintenance runner,
      # Turso-native extension registration if available.

    turso-timeseries-ext/
      # Turso-native extension implementation using `turso_ext`
      # if the API proves stable enough:
      # scalar functions, aggregates, vtabs/table functions.

    turso-timeseries-wasm/
      # wasm-bindgen wrapper around shared core for browser integration
      # beside @tursodatabase/database-wasm/sync-wasm.

    turso-timeseries-tests/
      # conformance and integration tests across native/wasm.

  examples/
    native-historian/
    wasm-historian/
    embedded-linux-gateway/
    preempt-rt-profile/
```

Do not split immediately unless forced. A pragmatic sequence is:

1. Start with `crates/turso-timeseries`.
2. Add `turso-timeseries-turso` when native integration becomes real.
3. Add `turso-timeseries-wasm` when browser integration becomes real.
4. Split `core`, `codec`, `catalog`, `influx`, `ext` only when LOC/compile boundaries justify it.

---

## 9. Dependency assumptions and reuse from InfluxDB

### 9.1 What to reuse directly, if compile/runtime checks pass

| Candidate | Use | Reuse likelihood | Notes |
|----------|-----|------------------|-------|
| Influx line protocol crates/code | Optional ingest compatibility | Medium | Useful if parser/serializer is sufficiently decoupled and WASM-compatible. |
| Arrow data model concepts or optional `arrow` crate | Export/import and future query acceleration | Medium | Keep optional; likely too heavy for default browser bundle. |
| Parquet concepts or optional `parquet` crate | Cold chunks/export | Low-medium | Native/server first; not v1 browser core. |
| DataFusion concepts or optional adapter | Analytical query acceleration | Low-medium | Server/native optional only until WASM size/runtime proven. |

### 9.2 What not to reuse directly

| Influx area | Reason |
|------------|--------|
| Full InfluxDB 3 server/core | It is database/server-shaped, not an embedded Turso extension. |
| Router/ingester/querier/compactor service graph | Too coupled to Influx deployment model. |
| Object-store lifecycle assumptions | Turso/browser v1 should keep state inside Turso DB/OPFS via Turso. |
| Python plugin/trigger system | Not aligned with pure Rust/browser/embedded constraints. |
| Full InfluxQL/Flux compatibility | Too broad; not needed for Hyperdrive historian v1. |

### 9.3 Concepts to borrow from Influx

- Line protocol ingest shape.
- Measurements/tags/fields mental model.
- WAL + immutable segment/compaction pipeline.
- TSM-like sorted blocks and per-series/per-column compression.
- Last-value cache.
- Distinct tag cache.
- Downsampling + retention as an operational pair.
- Series cardinality awareness.

---

## 10. Concepts to borrow from TimescaleDB

Borrow the product ergonomics, not PostgreSQL internals.

| Timescale concept | Turso-timeseries interpretation |
|------------------|---------------------------------|
| Hypertable | Catalog metadata describing a logical time-series table and its time/partition columns. |
| Chunks | `_tts_chunks` rows plus raw row tables or sealed segment BLOBs. |
| `time_bucket` | Native Rust bucket math and SQL scalar function if extension API allows. |
| Continuous aggregates | Explicit materialized rollup tables/chunks with refresh watermarks and invalidation tracking. |
| Retention policies | Job definitions + SQL/segment deletion routines. |
| Compression policies | Segment sealing/compaction and optional per-column compression. |
| Gap filling | Later table function/query helper; not v1. |
| Toolkit-style functions | `first`, `last`, `rate`, `increase`, approximate percentiles later. |
| Information views | `_tts_*` introspection views, not `timescaledb_information.*`. |

Do not copy Timescale names blindly where that implies compatibility. Prefer names such as:

```sql
SELECT tts_create_series_table(...);
SELECT tts_time_bucket(...);
SELECT tts_create_rollup_policy(...);
SELECT tts_run_maintenance(...);
```

Optionally provide aliases later if they do not create false compatibility expectations.

---

## 11. Storage model

### 11.1 V1 conservative layout

Start with Turso rows and indexes:

```sql
_tts_schema_version
_tts_series
_tts_retention_policies
_tts_rollup_policies
_tts_rollup_watermarks
_tts_invalidations
_tts_jobs

samples / tenant_samples
  series_id
  ts_ns or ts_ms
  value_real
  value_blob
  quality
```

Index:

```sql
CREATE INDEX ... ON samples(series_id, ts_ns DESC);
CREATE INDEX ... ON samples(ts_ns);
```

This is the safest integration path and easiest to test with Turso Sync.

### 11.2 V2 TSDB segment layout

Add Turso-owned segment BLOBs:

```sql
_tts_hypertables
_tts_chunks
_tts_segments
_tts_segment_columns
_tts_segment_stats
_tts_series_dict
_tts_ingest_wal
```

Segment properties:

- Stored as BLOB rows in Turso first.
- Checksummed.
- Versioned codec.
- Chunk-level min/max time.
- Per-column stats.
- Optional dictionary encoding for tags.
- Optional compression feature flags.

Avoid sidecar `.tsm` files in v1/v2 if sync is required. Sidecar files may become a native-only high-throughput feature later, but they require a separate atomicity/sync story.

---

## 12. Turso extension strategy

Turso supports loading **Turso-native** extensions using `load_extension('extension_name')`, and explicitly does not support SQLite `.so`/`.dll` loadable extensions. Therefore:

### 12.1 Native/server path

Investigate `turso_ext` for:

- scalar functions:
  - `tts_time_bucket`
  - `tts_time_bucket_ns`
  - `tts_first`
  - `tts_last`
  - `tts_rate`
- aggregate functions:
  - `tts_first_agg`
  - `tts_last_agg`
  - rollup aggregates if needed
- virtual tables/table functions:
  - `tts_scan`
  - `tts_chunk_stats`
  - `tts_last_value_cache`
  - `tts_rollup_status`

### 12.2 Browser path

Assume browser dynamic extension loading is not the first path. Prefer:

- static registration if Turso WASM exposes it;
- otherwise JS/WASM wrapper helpers that execute ordinary SQL and decode/encode segment BLOBs;
- keep browser package composition aligned with Turso's supported `database-wasm` / `sync-wasm` packaging.

### 12.3 Built-in Turso extensions to lean on

Use or interoperate with:

- Turso time extension where useful for nanosecond time conversion/truncation.
- Turso percentile extension for median/percentile if performance is acceptable.
- `generate_series` for gap-fill-like query generation experiments.
- UUID v7 functions for time-ordered identifiers if storing UUID BLOB keys.
- CDC/materialized views/custom index methods only after stability is confirmed.

---

## 13. Ingest API

### 13.1 Rust native

```rust
pub trait MetricSink {
    fn try_emit(&self, point: MetricPoint) -> Result<(), MetricBackpressure>;
}

pub async fn write_batch(
    conn: &mut TursoConnection,
    points: &[MetricPoint],
) -> Result<WriteStats>;

pub async fn write_line_protocol(
    conn: &mut TursoConnection,
    batch: &str,
) -> Result<WriteStats>;
```

### 13.2 Browser

```ts
await timeseries.writeBatch(db, points);
await timeseries.writeLineProtocol(db, lp);
await timeseries.runMaintenance(db, { budgetMs: 20 });
```

### 13.3 SQL surface

Where extension integration is available:

```sql
SELECT tts_write_lp('metrics,device=a temp=23.4 1778544000000000000');

SELECT tts_time_bucket(5 * dur_m(), ts_ns), avg(value_real)
FROM samples
WHERE series_id = ?
GROUP BY 1
ORDER BY 1;
```

---

## 14. Downsampling, rollups and retention

Downsampling is core, not optional polish.

Model:

```text
raw table/chunks
  -> dirty-bucket invalidation
  -> rollup policy worker
  -> rollup table/chunks
  -> retention policy deletes old raw/rollup data
```

V1 aggregates:

- count
- sum
- avg
- min
- max
- first
- last

V2 aggregates:

- stddev
- variance
- approximate percentile
- median
- histogram
- rate
- increase
- counter-reset-aware aggregates

Policy example:

```sql
SELECT tts_create_rollup_policy(
  'samples',
  'samples_5m',
  5 * dur_m(),
  'avg,min,max,count'
);

SELECT tts_add_retention_policy('samples', 7 * dur_d());
SELECT tts_add_retention_policy('samples_5m', 90 * dur_d());
```

Maintenance modes:

| Host | Maintenance execution |
|------|-----------------------|
| Native/server | Background async task or explicit maintenance command. |
| Browser | App/worker scheduled, usually after ingest, sync, idle time, or explicit user action. |
| PREEMPT_RT | Non-RT, low-priority maintenance thread. |
| Gateway | Scheduled service task with transaction budget. |
| Embassy MCU | Not applicable; gateway/host does maintenance. |

---

## 15. Sync and MVCC assumptions

### 15.1 Sync

For browser/local-first and embedded/offline scenarios:

- local writes must work offline;
- sync is owned by Turso;
- `turso-timeseries` must not open a side-channel sync protocol for segment files in v1;
- retention should run according to a documented host policy around `push`, `pull`, and `checkpoint`;
- retention jobs should support limited/batched deletes to keep transactions bounded.

### 15.2 MVCC / concurrent writes

Turso's MVCC/concurrent-write direction is useful but not a substitute for careful TSDB write design.

Invent/implement:

- idempotent chunk creation;
- monotonic chunk state transitions:
  - open
  - sealing
  - sealed
  - compacting
  - compacted
  - deleting
- transaction-scoped segment manifest updates;
- compare-and-swap style job claiming if multiple writers/maintenance workers exist;
- crash recovery for partially sealed/compacted chunks;
- dirty-bucket invalidations that can be safely replayed.

Do not allow compaction/downsampling to race unsafely with ingest.

---

## 16. Browser / OPFS assumptions

Turso's browser package uses OPFS and an async API. It also has bundler-specific exports and a worker/shared-memory architecture. Therefore:

- do not require raw OPFS access from `turso-timeseries-core`;
- do not require Rust `std::fs` in the browser path;
- do not assume dynamic extension loading;
- do not assume the IEC runtime, TSDB and Turso must compile into one WASM module;
- prefer separate WASM modules coordinated by JS/worker glue at first;
- avoid mandatory DataFusion/Arrow/Parquet in the browser bundle until size/performance are proven.

Initial browser architecture:

```text
Main UI
  -> IEC runtime WASM / runtime worker
  -> database worker or DB integration layer
  -> @tursodatabase/database-wasm
  -> turso-timeseries-wasm
  -> @tursodatabase/sync-wasm
  -> OPFS-backed DB
```

---

## 17. Embedded and real-time

| Context | Guidance |
|---------|----------|
| **Embedded Linux + `std`** | Full `turso` is viable; compile single binary; DB writer separated from runtime scan by bounded queue. |
| **PREEMPT_RT** | Use `SCHED_FIFO`/CPU affinity for scan thread; no Turso writes, compaction, downsampling or sync inside scan loop. |
| **Embassy MCU** | Do not embed Turso; use `no_std` IEC runtime core and emit compact telemetry to host/gateway. |
| **MCU + gateway** | MCU handles deterministic control; gateway runs Turso + TSDB + sync. |

RT-friendly principles:

- no blocking DB I/O in scan thread;
- no heap allocation in steady-state scan path where avoidable;
- preallocated bounded telemetry queues;
- drop/downsample low-priority telemetry under pressure;
- preserve alarms/events before debug traces;
- DB writer batches data outside RT priority.

---

## 18. Testing and CI

| Layer | What |
|-------|------|
| Unit tests | Bucket math, timestamp conversion, series identity, rollup windows, codec round-trips. |
| SQL tests | Migrations, schema versioning, idempotency, generated SQL snapshots. |
| Native Turso tests | Migrations + batch ingest + rollups against `turso` crate. |
| Extension tests | `turso_ext` scalar/aggregate/table-function tests if extension API is adopted. |
| Browser tests | Playwright/headless browser with `database-wasm`; add `sync-wasm` and Rust-generated wasm-bindgen coverage when those surfaces stabilize. |
| Sync tests | Simulate write/push/pull/retention/checkpoint ordering. |
| TSDB conformance | Timescale-inspired bucket/rollup tests and Influx-inspired line protocol/storage tests. |
| Load tests | Configurable 1M+ sample ingest, bounded transaction size, rollup correctness. |
| RT profile tests | Ensure scan loop only enqueues metrics and never awaits DB writes. |

**Authoritative phase → command matrix (exit gates for Phase 0–8):** [`docs/TESTING.md`](docs/TESTING.md). It defines `cargo test`, migration invariant coverage, the `integration-tests` feature path, and WASM CI placeholders so each roadmap phase ends in an automatable check.

### 18.1 Phase exit summary

| Phase | Minimal automation (see `docs/TESTING.md` for full table) |
|-------|-------------------------------------------------------------|
| 0 | `cargo test` |
| 1 | `cargo test` (includes embedded migration SQL invariants) |
| 2 | `cargo test` + `cargo test --features integration-tests` (DB-backed against pinned `turso`) + `rust-toolchain.toml` |
| 3–8 | Prior commands stay green; add extension, wasm, codec, fuzz, and integration jobs per phase in `docs/TESTING.md` |

---

## 19. Phased roadmap

### Phase 0 — Scaffold

- Workspace, README, current `time_bucket_ms`, basic tests.
- Document stack matrix.
- **Exit (tests):** `cargo test` — [`docs/TESTING.md`](docs/TESTING.md).

### Phase 1 — Conservative schema + migrations

- Add `_tts_schema_version`, `_tts_series`, retention, rollup policy and sample-table migration SQL.
- Keep default crate free of direct `turso` dependency.
- Add unit/snapshot tests.
- **Exit (tests):** `cargo test` (migration ordering, `SCHEMA_VERSION`, `CREATE TABLE` / `MAX(version, n)` invariants) — [`docs/TESTING.md`](docs/TESTING.md).

### Phase 2 — Native Turso integration

- Pin `rust-toolchain.toml`.
- Add `turso-timeseries-turso` or integration feature.
- Example native historian.
- Batch insert/query samples.
- CI native integration job.
- **Current implementation:** `native-turso` feature with a thin adapter for applying migrations and executing planned SQL batches against `turso = 0.6.0-pre.30`; `integration-tests` opens in-memory Turso, applies migrations idempotently, writes/query samples, and executes policy statements. Standalone E2E builds `turso-timeseries-ext` as a `cdylib`, initializes Turso with extension loading enabled, runs `SELECT load_extension(...)`, then round-trips inserts/selects.
- **Exit (tests):** same as Phase 1 plus `cargo test --features integration-tests` and `cargo test -p turso-timeseries-e2e --test standalone`. Native example and load-test p95 documentation remain follow-up work before treating Phase 2 as fully closed.

### Phase 3 — Turso extension spike

- Inspect `turso_ext` API and Turso `extensions/core` examples.
- Prototype scalar `tts_time_bucket`.
- Prototype aggregate/table function only if API surface is stable enough.
- Decide static vs loadable extension model for native.
- **Exit (tests):** Phase 0–2 gates plus extension-specific tests or documented fallback — [`docs/TESTING.md`](docs/TESTING.md).

### Phase 4 — Browser WASM spike

- Use `@tursodatabase/database-wasm` and `@tursodatabase/sync-wasm`.
- Compile TSDB wrapper/core to `wasm32`.
- Prove local OPFS persistence + push/pull/checkpoint compatibility.
- Document bundler requirements and COOP/COEP/SharedArrayBuffer implications if applicable.
- **Current implementation:** `e2e/browser` loads `@tursodatabase/database-wasm` through the Vite export and a separate generated timeseries WASM helper module in Chromium, then round-trips sample insert/select. Turso does not currently document browser-side third-party `SELECT load_extension(...)`, so dynamic SQL extension registration in browser remains open.
- **Exit (tests):** prior gates + `wasm32` CI job / smoke as listed in [`docs/TESTING.md`](docs/TESTING.md).

### Phase 5 — Downsampling + retention

- Rollup policy tables.
- Dirty-bucket invalidation.
- Batched rollup refresh.
- Batched retention deletes.
- Sync-order documentation.
- **Exit (tests):** prior gates + rollup/retention test modules — [`docs/TESTING.md`](docs/TESTING.md).

### Phase 6 — Segment storage / TSM-like codec

- Define segment BLOB format.
- Add codec round-trip tests.
- Store segment manifests transactionally in Turso.
- Add compaction state machine.
- Compare row layout vs segment layout performance.
- **Exit (tests):** prior gates + codec + compaction tests — [`docs/TESTING.md`](docs/TESTING.md).

### Phase 7 — Hyperdrive runtime integration

- Native gateway sink.
- Browser IEC runtime sink.
- PREEMPT_RT profile.
- Embassy telemetry-only profile.
- **Exit (tests):** prior gates + runtime wiring / bounded-queue tests — [`docs/TESTING.md`](docs/TESTING.md).

### Phase 8 — Hardening and release

- Fuzz bucket/codec/invalidation logic.
- Security review for SQL injection and untrusted line protocol.
- Benchmark docs.
- Semver 0.1.0.
- **Exit (tests):** prior gates + fuzz / release checklist — [`docs/TESTING.md`](docs/TESTING.md).

---

## 20. Risks and mitigations

| Risk | Mitigation |
|------|------------|
| Turso beta API churn | Pin versions; keep integration narrow; isolate adapters. |
| `turso_ext` instability | Treat extension integration as spike; preserve wrapper/SQL-template fallback. |
| Browser WASM packaging complexity | Use Turso's official WASM packages; avoid monolithic build initially. |
| Sync + retention conflicts | Document policy; store chunks in DB; add push/pull/retain tests. |
| Segment BLOBs too large for sync/OPFS | Start with row layout; benchmark; add chunk sizing limits. |
| High-cardinality tags | Series dictionary, cardinality metrics, last-value/tag-cache design. |
| Compaction races with ingest | Chunk state machine and transaction-scoped manifests. |
| Real-time jitter | Bounded queues; DB outside RT loop; budgeted transactions. |
| DataFusion/Arrow bloat | Keep optional native-only features until proven. |
| Influx crate coupling | Reuse only small, portable crates after WASM/native compile checks. |

---

## 21. Documentation deliverables

| Doc | Audience |
|-----|----------|
| `README.md` | Quick orientation and build matrix. |
| `PLAN.md` | Contributors and Hyperdrive architects. |
| `docs/TESTING.md` | Test tiers, `integration-tests` feature, phase exit gates, CI placeholders. |
| `docs/TURSO_EXTENSION_MODEL.md` | Notes from `turso_ext`, loadable/static extensions, browser fallback. |
| `docs/BROWSER_WASM.md` | database-wasm/sync-wasm packaging and constraints. |
| `docs/RETENTION_AND_SYNC.md` | Push/pull/checkpoint/retention ordering. |
| `docs/STORAGE_FORMAT.md` | Segment BLOB format once codec starts. |
| `docs/REALTIME_TARGETS.md` | PREEMPT_RT and Embassy guidance. |
| `docs/SQL_REFERENCE.md` | Tables, functions, policies and examples. |

---

## 22. Success criteria

How these map to `cargo test`, `--features integration-tests`, and CI placeholders: [`docs/TESTING.md`](docs/TESTING.md).

| Phase | Exit check |
|-------|------------|
| Phase 1 | Migrations apply cleanly and are no-op safe on second run. |
| Phase 2 | Native example inserts/query large batches without panic; query p95 documented. |
| Phase 3 | At least one Turso-native function is registered or a clear fallback decision is recorded. |
| Phase 4 | Browser build persists data through OPFS and can push/pull/checkpoint via sync-wasm. |
| Phase 5 | Rollups match raw aggregates within tolerance; retention does not corrupt catalog/sync expectations. |
| Phase 6 | Segment codec survives fuzz/round-trip tests; compaction is crash-safe in tests. |
| Phase 7 | IEC runtime emits telemetry to TSDB in native and browser; PREEMPT_RT path keeps DB out of scan loop. |
| Phase 8 | 0.1.0 release with semver surface and at least one Hyperdrive production path. |

---

## 23. Open questions

1. Exact package/crate mapping for browser Turso in Hyperdrive:
   - `@tursodatabase/database-wasm`
   - `@tursodatabase/sync-wasm`
   - bundler-specific imports such as Vite/Turbopack.
2. Whether Turso WASM exposes stable extension/function registration or requires wrapper-only integration first.
3. Whether to name public SQL functions `tts_*` only, or provide optional Timescale-like aliases.
4. Whether raw sample storage remains row-based through 0.1 or segment BLOB work begins before release.
5. How large segment BLOBs interact with sync, checkpointing and partial/local storage constraints.
6. Whether Turso materialized views can replace some custom rollup machinery later.
7. Whether CDC can drive invalidations/rollups later.
8. Whether UUID v7 BLOBs should be used for chunk/job ids.
9. How to model multi-writer ingest and maintenance job claiming under Turso MVCC.
10. How much of the IEC runtime core should be `no_std` for Embassy.

---

## 24. Related work elsewhere in Hyperdrive

**Related systems (upstream repo survey):** [docs/TIMESCALE_AND_INFLUX_STRUCTURES.md](docs/TIMESCALE_AND_INFLUX_STRUCTURES.md) compares TimescaleDB (`timescale/timescaledb` `main`) and InfluxDB 3 Core (`influxdata/influxdb` `main`) structure to `turso-timeseries` catalog/migrations and records explicit non-goals (wire protocols, SQL dialect parity).

- **blackhole:** PGlite + optional Timescale for Postgres-in-browser experiments. Not a dependency of this plan.
- **IEC 61131 runtime:** should emit metrics/events through host traits, not depend directly on Turso. Cross-cutting historian semantics (signal registry, quality, events, timestamp provenance): [`core/docs/HISTORIAN_MODEL.md`](../core/docs/HISTORIAN_MODEL.md), [`core/docs/TIME_MODEL.md`](../core/docs/TIME_MODEL.md), and [`core/docs/STANDARDS_ARCHITECTURE.md`](../core/docs/STANDARDS_ARCHITECTURE.md).
- **Browser app shell:** owns Turso WASM package loading, OPFS constraints and sync credentials.
- **Embedded gateway:** owns real-time scheduling, priority tuning and device transport.
- **Future OLAP adapter:** DuckDB/DataFusion/Parquet may query exported/compacted data but is not v1 core.

---

## 25. Reference links

- Turso in the Browser: https://turso.tech/blog/introducing-turso-in-the-browser
- Turso Sync: https://turso.tech/blog/introducing-databases-anywhere-with-turso-sync
- Turso SDK package matrix: https://docs.turso.tech/sdk/introduction
- Turso SQL extensions: https://docs.turso.tech/sql-reference/extensions
- Turso compatibility / Turso-specific extensions: https://docs.turso.tech/sql-reference/compatibility
- Turso repository: https://github.com/tursodatabase/turso
- Turso extension crate docs: https://docs.rs/turso_ext
- InfluxDB 3 storage-engine reference: https://docs.influxdata.com/influxdb3/clustered/reference/internals/storage-engine/

---

*End of plan — revise after the Turso extension spike and the first database-wasm/sync-wasm integration test.*
