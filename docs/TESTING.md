# Testing and CI (`turso-timeseries`)

This document is the **phase exit gate** matrix for the workspace. It complements [`PLAN.md`](../PLAN.md) §18 (Testing and CI) and the phased roadmap §19.

## Test layers

| Tier | Needs DB / Turso | Typical command | Purpose |
|------|------------------|-------------------|---------|
| **Unit** | No | `cargo test` | Pure Rust: bucket math, migration **static** invariants (`MIGRATIONS` order, `SCHEMA_VERSION`, SQL shape helpers, columnar segment encoding). |
| **Migration SQL invariants** | No | `cargo test` (same binary) | Parses embedded `include_str!` SQL without executing it: required `_tts_*` tables, `MAX(version, n)` per step, `CREATE TABLE` set, no duplicate versions. |
| **Integration (native)** | Yes, `turso` crate + pinned toolchain | `cargo test --features integration-tests` | Open a DB, apply migrations, assert catalog, ingest, policy, columnar segment, materialized rollup and retention paths. |
| **Standalone E2E** | Yes, `turso_core` + dynamic extension | `cargo test -p turso-timeseries-e2e --test standalone` | Build the `cdylib` extension, load it with `SELECT load_extension(...)`, run extension scalars, then insert/select samples. |
| **WASM / browser** | Custom Turso JS package + Vitest browser mode | `cd e2e/browser && npm run test:custom` | Build/install the Turso PR #6256 browser package, load `tts_extension.wasm` with `CREATE EXTENSION ... LANGUAGE wasm`, then round-trip samples, columnar segment rollups, materialized rollups and sandbox queries. |

**Default policy:** `cargo test` on the pinned stable toolchain must stay green **without compiling** the optional `turso` crate, so quick unit/migration checks remain lightweight.

## `integration-tests` feature

- **Toolchain:** `rust-toolchain.toml` pins Rust **1.95.0**, the latest stable verified for this repo on 2026-05-13.
- **Turso pin:** `turso = "=0.6.0-pre.30"` is optional and compiled by `native-turso` / `integration-tests`.
- **Cargo:** `crates/turso-timeseries/Cargo.toml` defines `native-turso` for the thin native adapter and `integration-tests` for DB-backed CI tests.
- **Integration test target:** `tests/integration.rs` is registered with `required-features = ["integration-tests"]`, so it is built only when you pass `--features integration-tests`. It opens an in-memory Turso DB, applies migrations idempotently, writes planned samples, queries them back, and executes retention/rollup policy statements.

### Cargo requirement

The latest Turso graph currently pulls crates whose manifests require Cargo support for Rust 2024 metadata. Cargo 1.82 fails before compilation with `feature edition2024 is required`. Use the pinned Rust 1.95.0 toolchain or newer.

## How to run locally

```bash
cd turso-timeseries

# Always-on unit + migration invariant tests (no Turso)
cargo test

# Native Turso integration tests
cargo test --features integration-tests

# Standalone native E2E: Turso core + documented dynamic extension loading
cargo test -p turso-timeseries-e2e --test standalone

# Optional native vtab repro: currently fails in Turso's dynamic VCreate path
TTS_ATTEMPT_VTAB=1 target/debug/tts-standalone-e2e

# Browser E2E: Vitest browser mode + custom Turso database-wasm + SQL-loaded timeseries WASM extension
cd e2e/browser
npm install
npx playwright install chromium
npm run test:custom
```

`npm run test:custom` requires a Rust toolchain with
`wasm32-wasip1-threads` stdlib available. If the script fails before compiling
the Turso branch, install the target with `rustup target add
wasm32-wasip1-threads` or switch to a toolchain distribution that ships it.

## Phase exit gates (PLAN §19)

Each phase **closes** when the listed commands pass (and any noted artefacts exist). Gates accumulate: later phases assume earlier tiers still pass in CI.

| Phase | Theme | Must pass / artefacts |
|-------|--------|------------------------|
| **0** | Scaffold | `cargo test` |
| **1** | Conservative schema + migrations | `cargo test` (includes migration SQL invariants); migrations committed under `crates/turso-timeseries/migrations/` |
| **2** | Native Turso integration | `cargo test`; `cargo test --features integration-tests` (DB apply + smoke ingest); `cargo test -p turso-timeseries-e2e --test standalone`; `rust-toolchain.toml` pinned; documented query p95 for batch example remains future load-test work |
| **3** | Turso extension spike | Phase 0–2 gates + extension/unit tests decided in `PLAN.md` / `docs/TURSO_EXTENSION_MODEL.md` (when added) |
| **4** | Browser WASM spike | Phase 0–2 gates + `wasm32` build job (placeholder acceptable until crate splits); bundler/OPFS docs |
| **5** | Rollups + retention | Phase 0–2 gates + rollup/retention correctness tests (new modules) |
| **6** | Segment / TSM codec | Prior gates + codec round-trip + compaction tests |
| **7** | Hyperdrive runtime wiring | Prior gates + bounded-queue / no-DB-in-scan tests where applicable |
| **8** | Hardening + release | Prior gates + fuzz targets / security review checklist per `PLAN.md` §22 |

**Browser CI:** `e2e/browser` uses Vitest browser mode with the Playwright Chromium provider and Vite. Browser SQL extension loading currently depends on Turso PR [#6256](https://github.com/tursodatabase/turso/pull/6256), so CI must either build that branch with `npm run test:custom` or pin to a future published `@tursodatabase/database-wasm` that includes the same `CREATE EXTENSION ... LANGUAGE wasm` support.

## Alignment with `PLAN.md` §22 (success criteria)

§22 states product-level exit checks (e.g. idempotent migrations, 1M samples). This file maps **how we prove** those in CI: §22 remains authoritative for *what* “done” means; §19 + this matrix define *which automated commands* must exist each phase.

## Adding a new migration (`0003_…`)

1. Add `migrations/0005_….sql` with `CREATE TABLE IF NOT EXISTS …` / idempotent DDL and `MAX(version, 5)` in `_tts_schema_version` update.
2. Add `pub const MIGRATION_0005_VERSION: u32 = 5;`, bump `SCHEMA_VERSION`, append to `MIGRATIONS`.
3. Extend `public_migration_version_constants_align_with_slice` (or replace with a small macro) so consts stay aligned with the slice.
4. Update expected table lists in `migrations.rs` tests if the catalog grows.
