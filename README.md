# turso-timeseries

A pure-Rust time-series extension for [Turso Database](https://github.com/tursodatabase/turso).

`turso-timeseries` provides TimescaleDB-inspired hypertables and rollups, InfluxDB-inspired line protocol ingest and TSM-style segment storage, and a Turso-native WASM extension artifact for browser/local-first use.

## Status

Experimental. The browser-native extension path follows Turso's unstable WASM extension design from [PR #6256](https://github.com/tursodatabase/turso/pull/6256).

## Targets

| Artifact | Crate / package | Use |
|----------|-----------------|-----|
| **Core** | `turso-timeseries-core` | Portable models, codecs, aggregates, line protocol (no Turso dependency) |
| **Catalog** | `turso-timeseries-catalog` | Embedded `_tts_*` migrations |
| **Native** | `turso-timeseries-native` | `Timeseries::install`, ingest, query, maintenance via the `turso` crate |
| **WASM extension** | `turso-timeseries-wasm-ext` | `CREATE EXTENSION turso_timeseries LANGUAGE wasm AS X'...'` |
| **JS helper** | `@hyperdrive-technology/turso-timeseries` | Hex loading and SQL wrappers (not the extension ABI itself) |
| **Facade** | `turso-timeseries` | Back-compat SQL planners + optional `native-turso` feature |

## Not WASI/WASIX

The browser extension is **not** a WASI or WASIX application. It is a raw WebAssembly module (`wasm32-unknown-unknown`) that exports the symbols expected by Turso's WASM extension ABI (`memory`, `turso_malloc`, `turso_ext_init`, scalar functions).

Do **not** use `wasm-bindgen` for the extension artifact. The optional `turso-timeseries-browser-wasm` crate remains a separate wasm-bindgen helper for legacy browser tests.

## Features

- Hypertables, chunks, and segment BLOBs inside Turso tables
- Line protocol ingest (`write_line_protocol`)
- `time_bucket` / duration parsing (core + WASM scalars)
- Internal aggregate state machines (count, sum, avg, min, max, first, last)
- Materialized rollups and retention helpers
- Stats tables and maintenance job catalog (PLAN-v2 migration `0005`)
- Optional Arrow / DataFusion adapter stubs (native/server only)

## Quick start (native)

```rust
use turso::Builder;
use turso_timeseries_native::Timeseries;

#[tokio::main]
async fn main() -> turso::Result<()> {
    let db = Builder::new_local("metrics.db").build().await?;
    let conn = db.connect()?;

    Timeseries::install(&conn).await?;
    Timeseries::create_hypertable(&conn, "metrics", "time", 60 * 60 * 1_000_000).await?;
    Timeseries::write_line_protocol(
        &conn,
        "metrics,device_id=a value=1.5 1778000000000000000",
    )
    .await?;

    let rows = Timeseries::read_points(&conn, "metrics", None, None).await?;
    println!("points: {}", rows.len());
    Ok(())
}
```

## Quick start (browser extension)

```ts
import { connect } from "@tursodatabase/database-wasm";
import { loadTimeseriesExtension } from "@hyperdrive-technology/turso-timeseries";

const db = await connect("metrics.db");
await loadTimeseriesExtension(db, { wasmUrl: "/turso_timeseries_ext.wasm" });
await db.exec("SELECT tts_version()");
await db.exec("SELECT time_bucket('5m', 1778000123456789)");
```

Build the WASM artifact:

```bash
rustup target add wasm32-unknown-unknown
cargo build -p turso-timeseries-wasm-ext --target wasm32-unknown-unknown --release
# output: target/wasm32-unknown-unknown/release/turso_timeseries_wasm_ext.wasm
```

## Workspace layout

See [PLAN-v2.md](PLAN-v2.md) for the full phased roadmap (Phases 0–13). Implementation status:

| Phase | Theme | Status |
|-------|--------|--------|
| 0 | README / standalone docs | Done |
| 1 | `turso-timeseries-core` | Done |
| 2 | Catalog + install | Done |
| 3 | Native write/read | Done |
| 4–5 | WASM scalars + manifest | Done (`turso-timeseries-wasm-ext`) |
| 6 | Aggregate engine | Done (core) |
| 7 | Rollups / invalidation | Partial (native maintenance + legacy planners) |
| 8 | Virtual tables | Partial (`turso-timeseries-ext` native vtab; WASM vtabs planned) |
| 9 | Join pushdown / stats | Partial (stats tables + cost estimator in core) |
| 10 | Maintenance engine | Done (native `run_maintenance`) |
| 11 | JS helper package | Done (`packages/turso-timeseries-js`) |
| 12 | Arrow / DataFusion | Stubs only |
| 13 | Sync repo | Documented in PLAN-v2 (out of scope here) |

## Testing

```bash
cargo test -p turso-timeseries-core
cargo test -p turso-timeseries
cargo test -p turso-timeseries --features integration-tests
cargo test -p turso-timeseries-native
```

See [docs/TESTING.md](docs/TESTING.md) for browser E2E and extension loading notes.

## Plans

- [PLAN-v2.md](PLAN-v2.md) — current canonical architecture and phases
- [PLAN.md](PLAN.md) — earlier Hyperdrive-oriented roadmap (historical)

## License

MIT OR Apache-2.0
