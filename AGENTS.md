## Cursor Cloud specific instructions

- The Cloud Agent image is configured by `.cursor/environment.json` and `.cursor/Dockerfile` (same pattern as [hyperdrive-technology/core](https://github.com/hyperdrive-technology/core) on branch `v2`).
- Rust toolchain version comes from `rust-toolchain.toml` (currently **1.95.0** with `clippy` and `rustfmt`).
- Default unit and migration checks: `cargo test` from the repo root.
- Native Turso integration: `cargo test --features integration-tests`.
- Standalone extension E2E: `cargo test -p turso-timeseries-e2e --test standalone`.
- Browser E2E (custom Turso WASM + Vitest): `cd e2e/browser && npm run test:custom` (requires `wasm32-wasip1-threads`, installed by the cloud `install` script).
- See [`docs/TESTING.md`](docs/TESTING.md) for the full phase gate matrix and CI expectations.
