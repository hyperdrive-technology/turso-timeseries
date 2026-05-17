# Turso extension API (notes from upstream README)

**Canonical source:** [Turso `extensions/core/README.md`](https://github.com/tursodatabase/turso/blob/main/extensions/core/README.md) (raw: `https://raw.githubusercontent.com/tursodatabase/turso/main/extensions/core/README.md`).

This file summarizes only what that README states; it does not extend upstream behavior.

---

## Standalone native Rust extension (`cdylib`)

The README positions the `turso_ext` crate as the way to build libraries that extend Turso and can be loaded “like traditional `sqlite3` extensions,” written in Rust.

**Crate layout and dependency**

- Add a library crate (e.g. `cargo new --lib extensions/your_crate_name`) and, for a **separate** extension crate, set `[lib] crate-type = ["cdylib", "lib"]`. The README explicitly says: “**NOTE:** Crate must be of type `cdylib` if you wish to link dynamically.”
- Depend on `turso_ext` via a **path** dependency (example shows `turso_ext = { path = "path/to/limbo/extensions/core", features = ["static", "vfs"] }` with a note that this is temporary until the crate is published). Optional workspace wiring shows `turso_ext` and extension crates as workspace path deps.
- Feature wiring shown: `static = ["turso_ext/static"]` at the extension crate level.

**`mimalloc` and `cfg(not(target_family = "wasm"))`**

- Under `[target.'cfg(not(target_family = "wasm"))'.dependencies]`, add `mimalloc = { version = "0.1", default-features = false }`. The README states mimalloc “is required if you intend on linking dynamically,” that it “is imported for you by the `register_extension` macro, so no configuration is needed,” but that it “must be added to your `Cargo.toml`” in that target-specific table.

**Registration and loading**

- Extensions are registered with the `register_extension!` macro (example lists `scalars`, `aggregates`, `vtabs`, `vfs`).
- The README notes: “Currently, any Derive macro used from this crate is required to be in the same file as the `register_extension` macro.”
- After `cargo build`, load the produced shared library via **CLI** `.load target/debug/libyour_crate_name` or **SQL** `SELECT load_extension('target/debug/libyour_crate_name')`.

**Allocator coordination when linking dynamically**

- The README warns that if you link with extensions dynamically, you must “coordinate the allocator with each extension” by either using **MiMalloc (the default)** or setting your global allocator in `macros/src/ext/mod.rs`.
- It shows an optional switch to `tikv_jemallocator::Jemalloc` on the global allocator, gated with `#[cfg(not(target_family = "wasm"))]` and `#[cfg(not(feature = "static"))]`, and adding `tikv-jemallocator` under `[target.'cfg(not(target_family = "wasm"))'.dependencies]` on the extension side. The surrounding text ties this to compiling the dynamic library and using the resulting `.so/.dylib/.dll` artifacts.

---

## WASM (`target_family = "wasm"`)

**What the README explicitly gates with `#[cfg(not(target_family = "wasm"))]`**

- The **global allocator override** example (MiMalloc vs jemalloc) uses `#[cfg(not(target_family = "wasm"))]` together with `#[cfg(not(feature = "static"))]`.
- The **extension `Cargo.toml` template** places **`mimalloc`** only under `[target.'cfg(not(target_family = "wasm"))'.dependencies]`.
- The **jemallocator** follow-up snippet for an extension also uses `[target.'cfg(not(target_family = "wasm"))'.dependencies]` for `tikv-jemallocator`.

**What the README still describes without a wasm/non-wasm split**

- A single **“Currently supported features”** list covers scalar functions (`scalar` macro), aggregates (`AggregateDerive` / `AggFunc`), virtual tables (`VTabModuleDerive` / `VTabCursor`), and VFS modules (`VfsExtension` / `VfsFile`). That list does **not** say “native only” or “excluded on wasm.”
- The **VFS** example in the README uses normal OS file APIs (`std::fs`, etc.); the README does not spell out wasm applicability for VFS.

**What this README does *not* document for WASM**

- It does **not** describe a wasm build of an extension crate, wasm `crate-type` expectations, how (or whether) `.load` / `load_extension` apply in a browser or wasm embedding, or any wasm-specific registration path.
- It does **not** state whether scalar, aggregate, or vtab APIs are supported, unsupported, or different when `target_family = "wasm"`.

Unless and until upstream documents wasm in the extension README, treat wasm
extension packaging in that README as **underspecified**.

### Browser WASM extension POC

For the browser E2E, this repo uses Turso PR
[#6256](https://github.com/tursodatabase/turso/pull/6256) as the current
prototype surface. That branch adds unstable WASM UDF / extension support to the
JavaScript WASM package:

```sql
CREATE EXTENSION turso_timeseries LANGUAGE wasm AS X'...';
```

The extension module used by `e2e/browser` is generated from WAT and follows the
branch's Turso-native manifest protocol:

- export `memory`;
- export `turso_malloc`;
- export `turso_ext_init(argc, argv) -> i64`;
- return a `TAG_TEXT` JSON manifest from `turso_ext_init`;
- list functions with `{ "name", "export", "narg" }`.

Current browser manifest:

```json
{
  "functions": [
    { "name": "tts_extension_loaded", "export": "tts_extension_loaded", "narg": 0 },
    { "name": "tts_time_bucket_ns", "export": "tts_time_bucket_ns", "narg": 2 }
  ]
}
```

The browser POC does **not** use `SELECT load_extension(...)`. It embeds the
WASM bytes as a SQL blob through the new `CREATE EXTENSION ... LANGUAGE wasm`
syntax from the PR branch.

---

## Virtual tables (design reference outside Turso)

For **how a time-oriented logical table can be exposed as a SQLite virtual table module** (partition interval, designated time column, insert routing into backing storage), see the independent **[Sqlite3_partitioner](https://github.com/nuuskamummu/Sqlite3_partitioner/)** project ([`CREATE VIRTUAL TABLE … USING partitioner(...)`](https://github.com/nuuskamummu/Sqlite3_partitioner/blob/main/README.md)). It targets **stock SQLite3** loadable extensions, not `turso_ext`, but it is a concrete **vtab-shaped** mental model when comparing to this repo’s **`_tts_*` catalog + plain tables** approach — summarized alongside Timescale/Influx in [`TIMESCALE_AND_INFLUX_STRUCTURES.md`](TIMESCALE_AND_INFLUX_STRUCTURES.md).

---

## Relation to `hyperdrive/turso-timeseries`

Today’s `turso-timeseries` has both:

- embedded SQL migrations and dependency-free Rust planning helpers; and
- `crates/turso-timeseries-ext`, a Turso-native `cdylib` extension loaded by the
  standalone E2E with `SELECT load_extension(...)`.

The native extension currently registers:

- scalar `tts_extension_loaded()`;
- scalar `tts_time_bucket_ns(ts_ns, width_ns)`;
- virtual table module `tts_hypertable`.

The intended native hypertable creation shape is:

```sql
CREATE VIRTUAL TABLE samples USING tts_hypertable(samples, 60000000000);
```

The vtab implementation exists in the extension crate, but the default
standalone E2E does not yet exercise dynamic `CREATE VIRTUAL TABLE`. Scalar
dynamic loading is verified.

### Dynamic cdylib vtab schema handoff finding

Repro fixture:

```bash
cargo test -p turso-timeseries-e2e --test standalone
TTS_ATTEMPT_VTAB=1 target/debug/tts-standalone-e2e
```

The first command stays green. The second intentionally attempts:

```sql
CREATE VIRTUAL TABLE samples USING tts_hypertable(samples, 60000000000);
```

On the current pinned `turso_core = 0.6.0-pre.30` dynamic extension path, this
fails before our cursor opens:

```text
ParseError("Failed to parse schema from virtual table module")
```

Follow-up comparison against confirmed working Turso examples found:

- upstream `extensions/tests/src/lib.rs` registers `kv_store` as a writable
  cdylib vtab and declares its schema as `CREATE TABLE x (...)`;
- upstream `testing/system/vtab.test` creates and writes that module through
  `CREATE VIRTUAL TABLE t USING kv_store`;
- a local minimal pre.30 probe extension with the same `VTabModuleDerive`
  pattern loads and creates successfully.

The `tts_hypertable` module now follows the same declared-schema convention
(`CREATE TABLE x (...)`) and keeps the returned table payload pointer-sized
(`type Table = Box<TtsHypertableTable>`). A raw ABI probe confirms this makes
the extension callback return a valid schema pointer. The pinned Turso
`VCreate` path still receives an empty schema string for this module, so the
gated standalone repro remains enabled as a focused upstream/runtime
compatibility check.

Earlier LLDB tracing at `turso_ext::vtabs::VTabModuleImpl::create` showed:

- built-in/static vtab modules return a valid schema string;
- our dynamically loaded `tts_hypertable` module returns `code = OK` and a
  non-null table pointer;
- the received `result.schema` pointer points at a NUL byte, while the actual
  schema bytes are nearby in memory.

That means the failure is below our SQL/schema text and before `VTable::open`,
`filter`, `insert`, or `column` can run. The remaining delta is in Turso's
runtime `VCreate` path for this dynamic module, not in the virtual-table cursor
logic.

Browser extension loading is separate: it is verified through the PR #6256
`CREATE EXTENSION ... LANGUAGE wasm` POC once the custom Turso WASM package can
be built locally.
