# Future sync package (`turso-timeseries-sync`)

Phase 13 of [PLAN-v2.md](../PLAN-v2.md) keeps sync orchestration **out of this repository**.

A future `hyperdrive-technology/turso-timeseries-sync` package would own:

- `push` / `pull` / `checkpoint` orchestration around Turso Sync
- Sync-aware compaction barriers and generation tracking
- Post-pull rollup invalidation refresh
- Multi-device conflict handling

This repo exposes catalog metadata (`_tts_invalidations`, `_tts_maintenance_jobs`, hypertable stats) so a sync layer can schedule refresh work without embedding sync protocol logic here.
