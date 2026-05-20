# Turso Timeseries Recommendations

Canonical architecture guidance for this repository. See [PLAN-v2.md](PLAN-v2.md) for phased delivery; this document defines **boundaries and naming**.

## Executive summary

Keep `turso-timeseries` standalone. The Turso **extension** owns SQL functions, virtual tables, catalog metadata, segment commit, query, rollup, retention and maintenance. **Ingest runtimes** own sockets, streams, timers, reconnects, buffering and backpressure — outside the extension.

```text
Influx-style internal series/cardinality model
+ Timescale-inspired SQL composition, rollups and hypertable ergonomics
+ Turso-native extension, virtual-table and local-first sync alignment
```

**Join rule:** `series_id` is the stable relational join key. Tags resolve series identity and cardinality; domain concepts (site, device, asset) live in ordinary Turso tables.

## Repository layout

```text
crates/
  turso-timeseries-core/           # portable TSDB logic (no Turso/Tokio/sockets)
  turso-timeseries-catalog/        # migrations + schema
  turso-timeseries-ext-wasm/       # CREATE EXTENSION ... LANGUAGE wasm
  turso-timeseries-ext-native/     # native load_extension / static link
  turso-timeseries-native/         # ergonomic Rust API for embedded Turso

  turso-timeseries-ingest-core/    # parser, batcher, EncodedBatch (no I/O)
  turso-timeseries-ingest-native/  # Tokio TCP/UDP → tts_write_batch
  turso-timeseries-ingest-wasm/    # wasm-bindgen helper for browser workers
  turso-timeseries-ingest-wasix/   # experimental WASIX compatibility only

packages/
  turso-timeseries-js/             # extension hex loader + SQL helpers
  turso-timeseries-ingest-js/       # Web Worker + WebSocket ingest
```

## Extension vs ingest

| Layer | Owns | Does not own |
|-------|------|----------------|
| **Extension** | `tts_write_batch`, vtabs, catalog, segments, maintenance | sockets, long-running loops, fetch/WebSocket |
| **Ingest runtime** | transport, flush policy, backpressure | direct catalog mutation without extension APIs |
| **Core** | codecs, aggregate states, series-key hash | Turso, OPFS, Tokio |

High-throughput write path:

```text
ingest runtime → EncodedBatch → SELECT tts_write_batch('metrics', X'...')
extension → chunk/segment BLOBs → COMMIT
```

## Write APIs (performance order)

1. **`tts_append_segment`** — pre-encoded segments (highest throughput)
2. **`tts_write_batch`** — `TTS_BATCH_V1` encoded batches (primary ingest path)
3. **`tts_write_lp_batch`** — Influx line protocol (medium)
4. **`tts_write_point`** — debugging / low rate only

## Catalog highlights

- `_tts_series` + `_tts_series_tags` + `_tts_tag_index` for identity and cardinality
- `_tts_segments` BLOB storage inside Turso (not sidecar TSM files)
- Optional `_tts_ingest_wal` for queryable recent batches (InfluxDB 3-style overlay)
- `_tts_tag_stats` for cardinality inspection

## Virtual tables

Composable vtabs keyed by `series_id` + time/bucket constraints: `tts_scan`, `tts_aggregate`, `tts_rollup`, `tts_last_value`, `tts_series`, `tts_series_tags`, `tts_chunks`.

Reject or price very high scans with no time/bucket range.

## Sync

Sync orchestration belongs in a future `turso-timeseries-sync` repo. Core works offline without sync packages.

## Implementation phases

| Phase | Focus |
|-------|--------|
| 0 | Standalone README; clarify ext-wasm vs ingest-wasm vs ingest-wasix |
| 1 | Core: `Point`, `SeriesId`, `EncodedBatch`, series-key hash, line protocol |
| 2 | Catalog install, series tags, cardinality policy |
| 3 | `tts_write_*` / `tts_append_segment`, segment encoding |
| 4 | Query vtabs + `series_id` pushdown |
| 5 | Maintenance, rollups, invalidations |
| 6 | Ingest runtimes (native + browser worker) |
| 7 | Browser `database-wasm` + OPFS smoke |
| 8 | Optional Arrow/DataFusion |

Full prose from the architecture review is preserved in git history; this file is the maintained summary for contributors.
