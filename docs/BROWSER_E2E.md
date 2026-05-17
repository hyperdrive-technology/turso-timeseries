# Browser E2E

The browser E2E harness lives in `e2e/browser`.

It validates the composition Turso documents today with Vitest browser mode
and the Playwright Chromium provider:

- `@tursodatabase/database-wasm` loaded through the Vite-specific export;
- a generated `tts_extension.wasm` module loaded into Turso with
  `CREATE EXTENSION ... LANGUAGE wasm AS X'...'`;
- migrations, inserts and selects round-tripped in a real Chromium browser.
- a sandbox page for running ad hoc SQL directly against the in-memory WASM
  Turso database.
- columnar hypertable segment tables (`_tts_segments` and
  `_tts_segment_columns`) populated from browser samples and used for rollup /
  downsampling checks.
- materialized rollup rows in `_tts_rollups` refreshed from columnar segment
  stats.

## Browser extension loading POC

Browser-side SQL extension loading depends on Turso PR
[#6256](https://github.com/tursodatabase/turso/pull/6256), specifically the
`glommer/limbo:turso-wasm-udf` branch. That branch adds unstable WASM UDF /
extension support to the JavaScript WASM package, including:

```sql
CREATE EXTENSION turso_timeseries LANGUAGE wasm AS X'...';
```

The browser harness generates a small Turso-native WASM extension from WAT in
`scripts/build-wasm-extension.mjs`. The module exports `turso_ext_init`, returns
a JSON manifest, and registers:

- `tts_extension_loaded()`
- `tts_time_bucket_ns(ts_ns, width_ns)`

The sandbox calls those functions through SQL after loading the extension. It no
longer directly instantiates the WASM module as a sidecar.

## Custom Turso package

To run the browser tests against the PR branch:

```bash
cd e2e/browser
npm install
npx playwright install chromium
npm run test:custom
```

`test:custom` runs `scripts/build-custom-turso-wasm.mjs`, which:

1. clones `https://github.com/glommer/limbo.git` at `turso-wasm-udf`;
2. builds `bindings/javascript` workspaces for `database-common`,
   `database-wasm-common` and `database-wasm`;
3. packs those packages and installs the local tarballs into `e2e/browser`;
4. runs the normal browser E2E.

The branch build requires a Rust toolchain with the
`wasm32-wasip1-threads` standard library installed. With rustup:

```bash
rustup target add wasm32-wasip1-threads
```

Homebrew Rust may list the target but still lack its stdlib. In that case use a
rustup-managed toolchain or another Rust distribution that includes the target
libraries.

The default published `@tursodatabase/database-wasm@0.6.0-pre.30` does not
support this POC path yet; `npm test` will only pass once the custom package has
been installed or the upstream feature lands in the published package.

References:

- https://turso.tech/blog/introducing-turso-in-the-browser
- https://docs.turso.tech/sql-reference/extensions
- https://github.com/tursodatabase/turso/pull/6256

Run:

```bash
cd e2e/browser
npm install
npx playwright install chromium
npm run test:custom
```
