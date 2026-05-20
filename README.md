# turso-timeseries

A pure-Rust time-series layer for [Turso Database](https://github.com/tursodatabase/turso).

Architecture follows [RECOMMENDATIONS.md](RECOMMENDATIONS.md): the **Turso extension** is database-facing (SQL + vtabs + segments); **ingest runtimes** own sockets, streams, and buffering outside the extension.

## Design principle

```text
stream / WebSocket / TCP
  → ingest runtime (ingest-core, ingest-native, ingest-js)
  → tts_write_batch / tts_append_segment
  → Turso extension commits catalog + segment BLOBs
```

`series_id` is the stable join key. Tags resolve series identity; domain tables (devices, sites) join on `series_id`.

## Targets

| Artifact | Crate / package |
|----------|-----------------|
| Core TSDB | `turso-timeseries-core` |
| Catalog | `turso-timeseries-catalog` |
| WASM extension | `turso-timeseries-ext-wasm` → `CREATE EXTENSION ... LANGUAGE wasm` |
| Native extension | `turso-timeseries-ext-native` → `load_extension` / static |
| Native API | `turso-timeseries-native` |
| Ingest (shared) | `turso-timeseries-ingest-core` |
| Ingest (native) | `turso-timeseries-ingest-native` |
| Ingest (browser helper) | `turso-timeseries-ingest-wasm`, `@hyperdrive-technology/turso-timeseries-ingest` |
| Ingest (WASIX, experimental) | `turso-timeseries-ingest-wasix` |
| JS extension loader | `@hyperdrive-technology/turso-timeseries` |

## Not WASI/WASIX (extension)

`turso-timeseries-ext-wasm` is a raw `wasm32-unknown-unknown` Turso extension (PR [#6256](https://github.com/tursodatabase/turso/pull/6256)), not a WASI app. WASIX is only for optional `ingest-wasix` compatibility.

## Write API performance order

1. `tts_append_segment` — pre-encoded segments  
2. `tts_write_batch` — `TTS_BATCH_V1` (`encode_batch` in core)  
3. `tts_write_lp_batch` — line protocol  
4. `tts_write_point` — debug / low rate  

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
        "runtime,task=MainTask value=1420 1778000000000000000",
    )
    .await?;
    Ok(())
}
```

## Quick start (ingest buffer)

```rust
use turso_timeseries_ingest_core::{FlushPolicy, IngestBuffer};

let mut buf = IngestBuffer::new(FlushPolicy::default());
if let Some(points) = buf.push_lp_text("m,t=a v=1 1778000000000000000", 0)? {
    // host calls Timeseries / tts_write_batch with encoded bytes
}
```

## Build WASM extension

```bash
rustup target add wasm32-unknown-unknown
cargo build -p turso-timeseries-ext-wasm --target wasm32-unknown-unknown --release
```

## Testing

```bash
cargo test -p turso-timeseries-core
cargo test -p turso-timeseries-ingest-core
cargo test -p turso-timeseries
cargo test -p turso-timeseries --features integration-tests
cargo test -p turso-timeseries-native
```

See [docs/TESTING.md](docs/TESTING.md) and [PLAN-v2.md](PLAN-v2.md).

## License

MIT OR Apache-2.0
