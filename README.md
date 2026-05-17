# turso-timeseries

Rust-first time-series helpers on top of **[Turso Database](https://github.com/tursodatabase/turso)** — the **pure Rust**, SQLite-compatible engine (`turso` / `turso_core` on crates.io). **Not** TimescaleDB-compatible and **not** targeting libSQL or `@libsql/client`.

Scope: bucketing, catalog/recipes, retention patterns, and SQL you run through **Turso’s Rust API** — same conceptual surface as the earlier feasibility note, different stack.

Roadmap and schema notes: [PLAN.md](PLAN.md). **Testing tiers and phase exit gates:** [docs/TESTING.md](docs/TESTING.md).

**Design references (not compatibility targets):** [docs/TIMESCALE_AND_INFLUX_STRUCTURES.md](docs/TIMESCALE_AND_INFLUX_STRUCTURES.md) surveys the TimescaleDB and InfluxDB 3 (`main`) GitHub layouts (policies, catalog, WAL/query/write crates) and maps concepts to this repo’s Turso-first direction. The same doc links **[Sqlite3_partitioner](https://github.com/nuuskamummu/Sqlite3_partitioner/)** as a **SQLite `CREATE VIRTUAL TABLE … USING partitioner(...)`** example for how a time-bucketed series can be modeled as a virtual table module (interval + partition column), distinct from this repo’s current `_tts_*` catalog + SQL migrations path.

**Engine status:** Turso Database is still **beta**; pin versions and read upstream release notes before production.

## Turso build matrix (Hyperdrive)

| Target | Turso stack | Notes |
|--------|-------------|--------|
| **Native** (`std`, host triples) | [`turso`](https://crates.io/crates/turso) (+ [`turso_core`](https://crates.io/crates/turso_core) where you need lower-level hooks) | Normal `cargo build` / embedded Linux. |
| **WASM** (`wasm32-unknown-unknown`) | **`turso-wasm`** + **`sync-wasm`** | Same IEC/runtime logic compiled to wasm; **DB + sync** come from your wasm-oriented Turso build (workspace members, path deps, or internal crates — whatever pins you use alongside upstream). |

This repo’s helpers are **engine-agnostic** at the type level: they compile for **both** native and wasm; **which** Turso artifacts you link is chosen by the **root** binary or wasm package (native `turso` vs wasm **`turso-wasm` / `sync-wasm`** split).

## Layout (planned)

| Crate / area | Role |
|--------------|------|
| `crates/turso-timeseries` | Shared types, SQL snippets, `time_bucket`-style helpers, migrations you run through **your** Turso dependency (native or wasm) |

This crate keeps native Turso integration behind optional features. **Application crates** can either execute the dependency-free SQL plans themselves, or enable `native-turso` to use the thin adapter pinned to `turso = 0.6.0-pre.30`. Browser packages should still use **`turso-wasm` + `sync-wasm`** (per your layout) and the dependency-free core surface.

## Targets and recommendations

| Deployment | IEC / control runtime | Storage | WASM? | Recommendation |
|------------|----------------------|---------|---------|----------------|
| **Native Linux/macOS / Windows** | Rust `std` + async runtime as needed | **`turso`** in-process | **No** | Single process; keep DB work off RT-critical threads. |
| **Linux + PREEMPT_RT** | RT scan on **FIFO / isolated CPU** | **`turso`** on normal threads | No | Never run heavy queries in the PLC scan; queue workers. |
| **Browser** | Rust → **`wasm32-unknown-unknown`** | **`turso-wasm`** + **`sync-wasm`** | **Yes** | One wasm app linking the wasm Turso stack + sync; pair with workers / OPFS per Turso’s browser guidance. |
| **Embedded Linux (`std`)** | One binary | **`turso`** | No | Single binary; flash-friendly I/O. |
| **Embassy / `no_std` MCU** | Firmware | **No full Turso** | N/A | Gateway runs Turso; MCU uses helpers only if you add a `core` slice later. |

### Notes

- **Native:** `turso` only — no wasm toolchains involved.
- **Browser / wasm32:** use your **`turso-wasm`** and **`sync-wasm`** build lines for the embedded DB + cloud/offline sync; keep `turso-timeseries` as shared logic compiled into that same wasm root.
- **Embedded / PREEMPT_RT:** same timing-domain split as always.

## Repo status

Scaffold + **Phase 2 native Turso integration path**:

- embedded SQL migrations (`crates/turso-timeseries/migrations/`, exposed via [`migrations`](crates/turso-timeseries/src/migrations.rs));
- dependency-free SQL planning helpers for ingest and policies;
- columnar hypertable/chunk/segment catalog tables with per-column blobs for `ts_ns`, `value_real` and `quality`;
- columnar segment planners for real-valued samples, materialized rollup refreshes over segment stats, and retention helpers that delete fully expired chunks;
- optional `native-turso` adapter that applies migrations and executes planned statements against the pinned native `turso` crate;
- DB-backed integration tests under `cargo test --features integration-tests`;
- standalone E2E tests that build and load `turso-timeseries-ext` with `SELECT load_extension(...)` and verify extension scalar functions;
- a native `tts_hypertable` virtual table module in `turso-timeseries-ext` with intended creation syntax `CREATE VIRTUAL TABLE samples USING tts_hypertable(samples, 60000000000)`;
- browser E2E wiring under `e2e/browser` for Turso PR #6256: build/install the custom `@tursodatabase/database-wasm`, load the generated timeseries WASM module with `CREATE EXTENSION ... LANGUAGE wasm AS X'...'`, and expose a browser sandbox for ad hoc SQL against the WASM DB.

Current caveat: scalar dynamic extension loading is verified in standalone
Turso. The native dynamic vtab implementation exists, but the pinned Turso
dynamic extension path does not yet successfully round-trip vtab schema strings
from the loaded `cdylib`, so standalone E2E does not exercise `CREATE VIRTUAL
TABLE` yet. Browser SQL extension loading requires the custom PR #6256 package
until that support is published upstream.

The crate also exposes dependency-free planning helpers for the conservative row layout:

```rust
use turso_timeseries::{
    MetricPoint, RollupAggregate, SeriesKey, plan_create_rollup_policy,
    plan_write_batch, time_bucket_ns,
};

let series = SeriesKey::new("temperature", [("device", "pump-1")])?;
let point = MetricPoint::real(series, 1_778_544_000_000_000_000, 23.4)?;

let batch = plan_write_batch(&[point]);
// Execute `batch.statements` in order inside a Turso transaction, binding
// each statement's `SqlValue` params to the matching `?` placeholders.

let bucket = time_bucket_ns(1_778_544_123_000_000_000, 60_000_000_000)?;
let policy = plan_create_rollup_policy(
    "_tts_samples",
    "samples_1m",
    60_000_000_000,
    &[RollupAggregate::Avg, RollupAggregate::Min, RollupAggregate::Max],
)?;
```

These helpers deliberately return SQL plus bind values instead of taking a concrete database connection, keeping the same core usable from native Turso, browser WASM bindings, and future extension/static-linking experiments.
