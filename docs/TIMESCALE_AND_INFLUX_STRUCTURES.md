# TimescaleDB & InfluxDB structures (design survey)

This note maps **public upstream layout and stated product behavior** to ideas for `turso-timeseries`. It is **not** a feature parity checklist and does **not** imply implementing PostgreSQL extension mechanics, the InfluxDB 3 server graph, or the Sqlite3_partitioner virtual table API.

**Surveyed branches (canonical repos):**

| System | Repository | Branch | Basis |
|--------|------------|--------|--------|
| TimescaleDB | [timescale/timescaledb](https://github.com/timescale/timescaledb) | `main` | [`README.md`](https://raw.githubusercontent.com/timescale/timescaledb/main/README.md) states it is a **PostgreSQL extension** for time-series and event data; user-facing examples reference hypertables, `time_bucket`, continuous aggregates, retention/compression in prose and SQL. |
| InfluxDB | [influxdata/influxdb](https://github.com/influxdata/influxdb) | **`main` = InfluxDB 3 Core** | [`README.md`](https://raw.githubusercontent.com/influxdata/influxdb/main/README.md) titles the tree **“InfluxDB 3 Core”** and documents **v3 on `main`**, **v2 on `main-2.x`**, **v1 on `master-1.x`**. This survey focuses on **`main` (v3)** for repo layout; v2/v1 differ materially (Go-centric trees on those branches — **TBD** for detailed layout here). |
| Sqlite3_partitioner | [nuuskamummu/Sqlite3_partitioner](https://github.com/nuuskamummu/Sqlite3_partitioner/) | `main` | Rust **loadable SQLite3 extension** for **automatic time-series partitioning** via **`CREATE VIRTUAL TABLE … USING partitioner(...)`** (interval such as `1 hour` / `1 day`, columns, one **`partition_column`** on a timestamp stored as `TEXT`). [Upstream README](https://github.com/nuuskamummu/Sqlite3_partitioner/blob/main/README.md) documents create/insert and calls the project **experimental** (shadow tables, indexing limitations). **Not** a Turso or `turso_ext` crate — a **mental model** for “time-partitioned logical table as a vtab module” vs this repo’s Turso-first **`_tts_*` catalog + migrations** approach. |

---

## TimescaleDB — high-level layout

### Product shape (PostgreSQL extension)

Upstream describes TimescaleDB as a **PostgreSQL extension** (not a standalone database). Installation targets PostgreSQL library paths (`install(TARGETS ... DESTINATION ${PG_PKGLIBDIR})` in [`src/CMakeLists.txt`](https://raw.githubusercontent.com/timescale/timescaledb/main/src/CMakeLists.txt)). Root [`CMakeLists.txt`](https://raw.githubusercontent.com/timescale/timescaledb/main/CMakeLists.txt) wires `pg_config`, regression checks against PostgreSQL, and extension packaging — all **PostgreSQL-coupled**.

The public [`README.md`](https://raw.githubusercontent.com/timescale/timescaledb/main/README.md) illustrates:

- Hypertables (upstream Timescale examples use `CREATE TABLE ... WITH (tsdb.hypertable)`; **turso-timeseries** uses native Turso/SQLite extension surfaces instead, currently `CREATE VIRTUAL TABLE ... USING tts_hypertable(...)` for the vtab-shaped path and catalog SQL for browser fallback paths),
- `time_bucket` in analytical queries,
- Continuous aggregates (`CREATE MATERIALIZED VIEW ... WITH (timescaledb.continuous)` and `add_continuous_aggregate_policy`),
- Columnstore / compression-oriented workflows (`convert_to_columnstore`, `timescaledb.enable_direct_compress_insert` in examples).

### `src/` directory modules (repo structure)

[`src/README.md`](https://raw.githubusercontent.com/timescale/timescaledb/main/src/README.md) points to feature areas:

- [`src/adts/`](https://github.com/timescale/timescaledb/tree/main/src/adts) — abstract data types (per `src/README.md`).
- [`src/bgw/`](https://github.com/timescale/timescaledb/tree/main/src/bgw) — background worker scheduler; design in [`src/bgw/README.md`](https://raw.githubusercontent.com/timescale/timescaledb/main/src/bgw/README.md) (jobs table, `schedule_interval`, stats catalog, **PostgreSQL background worker** execution model).
- [`src/bgw_policy/`](https://github.com/timescale/timescaledb/tree/main/src/bgw_policy) — built from `policy.c`, `chunk_stats.c` per [`src/bgw_policy/CMakeLists.txt`](https://raw.githubusercontent.com/timescale/timescaledb/main/src/bgw_policy/CMakeLists.txt).
- [`src/loader/`](https://github.com/timescale/timescaledb/tree/main/src/loader) — multi-version loader (per `src/README.md`).
- [`src/ts_catalog/`](https://github.com/timescale/timescaledb/tree/main/src/ts_catalog) — catalog-oriented compilation units including `catalog.c`, `metadata.c`, `continuous_agg.c`, `continuous_aggs_jobs_refresh_ranges.c`, `continuous_aggs_watermark.c`, `compression_chunk_size.c`, `compression_settings.c`, `chunk_rewrite.c`, `chunk_column_stats.c` per [`src/ts_catalog/CMakeLists.txt`](https://raw.githubusercontent.com/timescale/timescaledb/main/src/ts_catalog/CMakeLists.txt).
- [`src/planner/`](https://github.com/timescale/timescaledb/tree/main/src/planner), [`src/nodes/`](https://github.com/timescale/timescaledb/tree/main/src/nodes) — present as subdirectories under `src/` (GitHub API listing for `src/` on `main`); together with `import/*.c` planner-related sources in [`src/CMakeLists.txt`](https://raw.githubusercontent.com/timescale/timescaledb/main/src/CMakeLists.txt), this is **PostgreSQL planner / executor integration**.
- Other top-level `src/` subdirectories on `main`: `compat`, `import`, `net`, `telemetry`, `with_clause` (from [GitHub `src` tree API](https://api.github.com/repos/timescale/timescaledb/contents/src?ref=main)).

Core extension C files at `src/` root (from [`src/CMakeLists.txt`](https://raw.githubusercontent.com/timescale/timescaledb/main/src/CMakeLists.txt)) include hypertable/chunk/dimension/partitioning (`hypertable.c`, `chunk*.c`, `dimension*.c`, `partitioning.c`, `partition_chunk.c`), **time bucketing** (`time_bucket.c`, `time_utils.c`), tuple routing and scans (`chunk_tuple_routing.c`, `chunk_scan.c`), caches, triggers, copy path, gapfill, histogram, and extension bootstrap (`extension.c`, `init.c`).

**Information / introspection schema:** not enumerated file-by-file in this pass. Tiger Data / Timescale document `timescaledb_information` style views in product docs; exact view definitions live in SQL/catalog sources under the full repo — **TBD** for a definitive file list without deeper tree walk.

### PostgreSQL-specific vs conceptually portable

| PostgreSQL-specific (tied to PG extension/runtime) | Conceptually portable (usable as design vocabulary) |
|---------------------------------------------------|------------------------------------------------------|
| `MODULE` library install to `PG_PKGLIBDIR`, `pg_config`, planner `import/*.c` hooks, background worker scheduler tied to postmaster | Hypertable *idea*: logical table + time partitioning + chunk metadata |
| PostgreSQL `MATERIALIZED VIEW` + `timescaledb.continuous` machinery, PG catalog integration | Continuous aggregate *idea*: incremental rollup + refresh window + policy |
| Server-side background workers and PG job catalog semantics | Retention / reorder / compression *policies* as scheduled maintenance jobs |
| `time_bucket` as a PG extension function implementation | Time bucketing as a **SQL surface** or Rust helper over `ts` + interval |
| Chunk creation as child tables in PG inheritance/partitioning | Chunk metadata, open vs sealed ranges, pruning by time |

---

## Sqlite3_partitioner — SQLite virtual table (reference only)

[Sqlite3_partitioner](https://github.com/nuuskamummu/Sqlite3_partitioner/) is a small **Rust** project that adds a **`partitioner` virtual table module** to stock **SQLite3** (`.load` of a `.so` / `.dylib` from [releases](https://github.com/nuuskamummu/Sqlite3_partitioner/releases)). It is useful here as a **reference for how to think about time-series virtual tables**, not as something to embed or match API-for-API.

**Shape (from upstream README):**

- Declare a logical table with **`CREATE VIRTUAL TABLE … USING partitioner(`** *interval*, *column defs including one `timestamp partition_column`*, **`)`** — e.g. `1 hour` / `1 day` style intervals and a designated time column.
- **Inserts** go through the module, which routes rows into **partitioned storage** (shadow tables); upstream documents **indexing** limitations and a separate usage guide on their [GitHub Pages site](https://nuuskamummu.github.io/Sqlite3_partitioner/).

**How this relates to `turso-timeseries`:**

- **Similar problem:** bucket rows in time, keep a stable **SQL surface** (`INSERT`/`SELECT` on one name), hide physical splits behind a module.
- **Different stack:** classic SQLite3 C extension / loadable module, not **Turso**’s [`turso_ext`](https://github.com/tursodatabase/turso/blob/main/extensions/core/README.md) **`VTabModuleDerive`** path ([`docs/TURSO_EXTENSION_API.md`](TURSO_EXTENSION_API.md)). This repo now has both **ordinary catalog tables** (`_tts_hypertables`, `_tts_chunks`, `_tts_segments`, …) and an initial `turso-timeseries-ext` vtab module named `tts_hypertable`; scalar loading is verified, while dynamic vtab execution is still blocked by the pinned Turso schema handoff path.
- **Upstream caveats:** experimental, visible shadow tables, datetime parsing edge cases — treat as design inspiration only.

---

## InfluxDB 3 Core (`main`) — high-level layout

### Product shape (from README)

[`README.md`](https://raw.githubusercontent.com/influxdata/influxdb/main/README.md) states **InfluxDB 3 Core** is built with **Rust**, **Apache Arrow**, **DataFusion**, **Parquet**; highlights diskless/object-storage deployment, **line protocol** write, **SQL + InfluxQL + Flight SQL** query paths, HTTP on port **8181**, and compatibility with **1.x/2.x write APIs** and **1.x InfluxQL query API**.

### Workspace / crate layout (storage, metadata, query, server)

The root [`Cargo.toml`](https://raw.githubusercontent.com/influxdata/influxdb/main/Cargo.toml) workspace lists, among others:

- **Process & server shell:** `influxdb3`, `influxdb3_process`, `influxdb3_server`, `influxdb3_startup`, `influxdb3_shutdown`, `influxdb3_commands`.
- **Write path & durability:** `influxdb3_write`, `influxdb3_wal` — `influxdb3_write` depends on `influxdb3_wal`, `influxdb3_catalog`, `parquet_file`, `object_store`, `datafusion`, `influxdb-line-protocol`, etc. ([`influxdb3_write/Cargo.toml`](https://raw.githubusercontent.com/influxdata/influxdb/main/influxdb3_write/Cargo.toml)).
- **Catalog:** `influxdb3_catalog` — depends on `schema`, `object_store`, `influxdb3_wal`, `influxdb3_authz`, line protocol crate ([`influxdb3_catalog/Cargo.toml`](https://raw.githubusercontent.com/influxdata/influxdb/main/influxdb3_catalog/Cargo.toml)).
- **WAL:** `influxdb3_wal` — depends on `object_store`, line protocol, `schema`, `bitcode`, etc. ([`influxdb3_wal/Cargo.toml`](https://raw.githubusercontent.com/influxdata/influxdb/main/influxdb3_wal/Cargo.toml)).
- **Query execution:** `influxdb3_query_executor` plus large `core/iox_query`, `core/datafusion_util`, `core/parquet_file`, `core/influxdb_line_protocol`, Flight/gRPC stacks under `core/` (workspace member list in same `Cargo.toml`).
- **AuthZ / IDs / types:** `influxdb3_authz`, `influxdb3_id`, `influxdb3_types`.
- **Cache & system tables:** `influxdb3_cache`, `influxdb3_system_tables`, `core/iox_system_tables`.
- **Line protocol (ingest format):** `core/influxdb_line_protocol`, `core/mutable_batch_lp`.

**Organizations / buckets (v3 vs v2):** v2’s product model (orgs, buckets, tokens) is prominent in v2 docs; for **v3 Core** this survey did not trace org/bucket representation to a single canonical type — mark **TBD** for “where in `influxdb3_catalog` / HTTP API structs org & bucket live” without reading those modules end-to-end.

### Engine-specific vs conceptually portable

| Engine-specific (InfluxDB 3 / IOx-shaped stack) | Conceptually portable |
|------------------------------------------------|------------------------|
| DataFusion fork + full distributed query service graph | **Line protocol** parsing as an optional ingest adapter |
| `object_store` + Parquet file layout + WAL crate semantics | Append **segments** / sealed files + object-key naming discipline (concept only) |
| HTTP/Flight API surface, authz crate, Python processing engine | **Last-value** / metadata cache patterns; **async refresh** of rollups |
| `influxdb3_server` process model | Explicit **writer** vs **query** resource isolation (pattern) |

---

## Mapping table (concepts → Timescale | Influx 3 | turso-timeseries direction)

| Concept | TimescaleDB approach (surveyed) | InfluxDB 3 Core approach (surveyed) | turso-timeseries direction |
|--------|----------------------------------|--------------------------------------|-----------------------------|
| **Primary store** | PostgreSQL tables; hypertable/chunk machinery in extension C code (`hypertable.c`, `chunk*.c`, etc. per [`src/CMakeLists.txt`](https://raw.githubusercontent.com/timescale/timescaledb/main/src/CMakeLists.txt)) | Parquet + object store + WAL (`influxdb3_write`, `influxdb3_wal`, `parquet_file` per workspace / crate deps) | **Turso-native:** SQLite tables/BLOBs first; optional later segment files still behind Turso durability/sync ([`PLAN.md`](../PLAN.md) §7, §3). |
| **Series / metric identity** | SQL schema + indexes (README examples use columns like `sensor_id`); internal catalog in `ts_catalog` | Line protocol → `influxdb-line-protocol`, `schema`, `data_types` in core | **`_tts_series`** (`metric_name`, `tags_json`) + **`_tts_samples`** ([`0001_catalog_and_samples.sql`](../crates/turso-timeseries/migrations/0001_catalog_and_samples.sql)); future tag dictionary **TBD**. |
| **Time bucketing** | `time_bucket()` in SQL ([`README.md`](https://raw.githubusercontent.com/timescale/timescaledb/main/README.md)); implementation `time_bucket.c` | SQL/DataFusion time functions (engine-wide; exact UDF surface **TBD** without `iox_query` audit) | Rust `time_bucket`-style helpers + SQL templates ([`PLAN.md`](../PLAN.md) §4 goals, §10). |
| **Rollups / downsampling** | Continuous aggregates + refresh policies in README SQL; `continuous_agg*.c` under `ts_catalog` | **TBD:** continuous-query equivalent in v3 (likely query + materialization pipeline, not surveyed file-deep) | **`_tts_rollup_policies`**, **`_tts_rollup_watermarks`**, **`_tts_invalidations`**, **`_tts_jobs`** ([`0002_policies_and_jobs.sql`](../crates/turso-timeseries/migrations/0002_policies_and_jobs.sql)); aligns with PLAN “dirty-bucket invalidation”. |
| **Retention** | README / product docs describe retention policies; `bgw_policy` includes `policy.c` (exact policy matrix **TBD** at C file level) | Object lifecycle + catalog policies (**TBD** detail) | **`_tts_retention_policies`** ([`0002_policies_and_jobs.sql`](../crates/turso-timeseries/migrations/0002_policies_and_jobs.sql)). |
| **Compression / columnstore** | README columnstore / `convert_to_columnstore`; `compression_*.c` in `ts_catalog` | Columnar Parquet + encoding (Parquet ecosystem) | Turso/SQLite compression and future sealed-segment codecs ([`PLAN.md`](../PLAN.md) §4, codec crate sketch); **not** PG columnstore. |
| **Background maintenance** | PostgreSQL bgw scheduler (`src/bgw/README.md`) | Async Tokio services in server/write crates (**TBD** single entrypoint) | **`_tts_jobs`** queue + application-side runner (no postmaster); PLAN §7 “retention/maintenance engine”. |
| **Ingest API** | Standard PostgreSQL insert/copy | Line protocol + HTTP; compatibility claims for 1.x/2.x writes in v3 README | Optional **line protocol** adapter crate later ([`PLAN.md`](../PLAN.md) skeleton `turso-timeseries-influx`); primary path: Rust batch API / SQL. |
| **Query API** | PostgreSQL SQL + extension functions | SQL, InfluxQL, Flight SQL per README | **Turso SQL** + Rust helpers; no InfluxQL/Flux server ([`PLAN.md`](../PLAN.md) §5 non-goals). |
| **Catalog / introspection** | `ts_catalog/*.c` + PG catalog integration | `influxdb3_catalog`, `influxdb3_system_tables` | **`_tts_*` tables**; future introspection views with **non**-Timescale names ([`PLAN.md`](../PLAN.md) §10). |

---

## Explicit non-goals (this repository)

Aligned with [`PLAN.md`](../PLAN.md) §5 and README positioning:

- **Wire protocol compatibility:** InfluxDB HTTP/Flight/1.x/2.x APIs, TimescaleDB/PostgreSQL wire protocols.
- **SQL dialect compatibility:** Full PostgreSQL, TimescaleDB extension SQL, InfluxQL, Flux as supported query surfaces.
- **Embedding upstream servers:** InfluxDB 3 server, PostgreSQL, or TimescaleDB as libraries inside this crate.
- **Replicating Timescale background worker or PG planner hooks** inside Turso — instead, **explicit Rust job runners** over `_tts_jobs` and Turso transactions.
- **Claiming org/bucket/token parity** with InfluxDB Cloud/OSS v2 — **TBD** if any subset is later mapped for historian use cases.

---

## Sources (traceability)

| Claim | Source |
|-------|--------|
| TimescaleDB = PostgreSQL extension | [`timescaledb/README.md`](https://raw.githubusercontent.com/timescale/timescaledb/main/README.md) |
| Hypertable / `time_bucket` / continuous aggregate examples | Same README SQL blocks |
| `src/` CMake structure, `ts_catalog` file set, `bgw_policy` sources | [`src/CMakeLists.txt`](https://raw.githubusercontent.com/timescale/timescaledb/main/src/CMakeLists.txt), [`src/ts_catalog/CMakeLists.txt`](https://raw.githubusercontent.com/timescale/timescaledb/main/src/ts_catalog/CMakeLists.txt), [`src/bgw_policy/CMakeLists.txt`](https://raw.githubusercontent.com/timescale/timescaledb/main/src/bgw_policy/CMakeLists.txt) |
| Background worker scheduler behavior | [`src/bgw/README.md`](https://raw.githubusercontent.com/timescale/timescaledb/main/src/bgw/README.md) |
| `src/` subdirectories list | [GitHub API `.../contents/src?ref=main`](https://api.github.com/repos/timescale/timescaledb/contents/src?ref=main) |
| InfluxDB 3 Core positioning, branch map, tech stack bullets | [`influxdb/README.md`](https://raw.githubusercontent.com/influxdata/influxdb/main/README.md) |
| Workspace members / crate graph for write, wal, catalog | [`influxdb/Cargo.toml`](https://raw.githubusercontent.com/influxdata/influxdb/main/Cargo.toml), [`influxdb3_write/Cargo.toml`](https://raw.githubusercontent.com/influxdata/influxdb/main/influxdb3_write/Cargo.toml), [`influxdb3_catalog/Cargo.toml`](https://raw.githubusercontent.com/influxdata/influxdb/main/influxdb3_catalog/Cargo.toml), [`influxdb3_wal/Cargo.toml`](https://raw.githubusercontent.com/influxdata/influxdb/main/influxdb3_wal/Cargo.toml) |
| Sqlite3_partitioner vtab API & status | [Repository README](https://github.com/nuuskamummu/Sqlite3_partitioner/blob/main/README.md), [usage docs](https://nuuskamummu.github.io/Sqlite3_partitioner/usage/) |
| turso-timeseries schema & roadmap | [`PLAN.md`](../PLAN.md), [`0001_catalog_and_samples.sql`](../crates/turso-timeseries/migrations/0001_catalog_and_samples.sql), [`0002_policies_and_jobs.sql`](../crates/turso-timeseries/migrations/0002_policies_and_jobs.sql) |

---

*Document version: 2026-05-12. Refresh when upstream `main` layout changes materially.*
