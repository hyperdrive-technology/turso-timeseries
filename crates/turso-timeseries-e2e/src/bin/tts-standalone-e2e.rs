use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use turso_core::{Database, DatabaseOpts, MemoryIO, OpenFlags, StepResult, IO};
use turso_timeseries::migrations::MIGRATIONS;

fn main() {
    run();
    std::process::exit(0);
}

fn run() {
    let extension = build_extension_cdylib();
    let io: Arc<dyn IO> = Arc::new(MemoryIO::new());
    let db = Database::open_file_with_flags(
        io.clone(),
        ":memory:",
        OpenFlags::Create,
        DatabaseOpts::new().turso_cli(),
        None,
    )
    .expect("open standalone Turso DB with extension loading enabled");
    let conn = db.connect().expect("connect Turso DB");

    for migration in MIGRATIONS {
        execute_script(io.as_ref(), &conn, migration.sql);
    }

    execute(
        io.as_ref(),
        &conn,
        &format!("SELECT load_extension('{}')", sql_path(&extension)),
    );
    assert_eq!(
        query_one_text(io.as_ref(), &conn, "SELECT tts_extension_loaded()"),
        "turso-timeseries-ext"
    );
    assert_eq!(
        query_one_i64(io.as_ref(), &conn, "SELECT tts_time_bucket_ns(1500, 1000)"),
        1000
    );
    if std::env::var_os("TTS_ATTEMPT_VTAB").is_some() {
        execute(
            io.as_ref(),
            &conn,
            "CREATE VIRTUAL TABLE samples USING tts_hypertable(samples, 60000000000)",
        );
    }

    execute(
        io.as_ref(),
        &conn,
        "INSERT OR IGNORE INTO _tts_series (metric_name, tags_json) \
         VALUES ('temperature', '{\"device\":\"pump-1\"}')",
    );
    execute(
        io.as_ref(),
        &conn,
        "INSERT INTO _tts_samples (series_id, ts_ns, value_real, quality) \
         SELECT series_id, 1778544000000000000, 23.4, 7 \
         FROM _tts_series \
         WHERE metric_name = 'temperature' AND tags_json = '{\"device\":\"pump-1\"}'",
    );

    assert_eq!(
        query_one_i64(io.as_ref(), &conn, "SELECT COUNT(*) FROM _tts_samples"),
        1
    );
    assert_eq!(
        query_one_text(
            io.as_ref(),
            &conn,
            "SELECT metric_name \
             FROM _tts_series \
             WHERE series_id = (SELECT series_id FROM _tts_samples LIMIT 1)"
        ),
        "temperature"
    );
    assert_eq!(
        query_one_text(
            io.as_ref(),
            &conn,
            "SELECT tags_json \
             FROM _tts_series \
             WHERE series_id = (SELECT series_id FROM _tts_samples LIMIT 1)"
        ),
        "{\"device\":\"pump-1\"}"
    );
    assert_eq!(
        query_one_i64(
            io.as_ref(),
            &conn,
            "SELECT tts_time_bucket_ns(ts_ns, 60000000000) FROM _tts_samples"
        ),
        1778544000000000000
    );
}

fn build_extension_cdylib() -> PathBuf {
    let status = Command::new(env!("CARGO"))
        .args(["build", "-p", "turso-timeseries-ext"])
        .status()
        .expect("spawn cargo build for extension");
    assert!(status.success(), "extension cdylib build failed");

    let file_name = format!(
        "{}turso_timeseries_ext{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_SUFFIX
    );
    workspace_root()
        .join("target")
        .join("debug")
        .join(file_name)
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn sql_path(path: &Path) -> String {
    path.to_string_lossy().replace('\'', "''")
}

fn execute_script(io: &dyn IO, conn: &Arc<turso_core::Connection>, sql: &str) {
    for statement in sql.split(';').map(str::trim).filter(|s| !s.is_empty()) {
        let normalized = statement
            .lines()
            .map(str::trim)
            .filter(|line| !line.starts_with("--"))
            .collect::<Vec<_>>()
            .join(" ");
        let upper = normalized.to_ascii_uppercase();
        if upper.starts_with("CREATE INDEX") || upper.starts_with("CREATE UNIQUE INDEX") {
            continue;
        }
        execute(io, conn, statement);
    }
}

fn execute(io: &dyn IO, conn: &Arc<turso_core::Connection>, sql: &str) {
    let _ = io;
    conn.execute(sql).expect(sql);
}

fn query_one_i64(io: &dyn IO, conn: &Arc<turso_core::Connection>, sql: &str) -> i64 {
    let stmt = query_one_row(io, conn, sql);
    stmt.row().expect("row").get::<i64>(0).unwrap()
}

fn query_one_text(io: &dyn IO, conn: &Arc<turso_core::Connection>, sql: &str) -> String {
    let stmt = query_one_row(io, conn, sql);
    stmt.row().expect("row").get::<String>(0).unwrap()
}

fn query_one_row(
    io: &dyn IO,
    conn: &Arc<turso_core::Connection>,
    sql: &str,
) -> turso_core::Statement {
    let mut stmt = conn.prepare(sql).expect(sql);
    loop {
        match stmt.step().expect(sql) {
            StepResult::Row => return stmt,
            StepResult::IO => io.step().expect("run IO"),
            StepResult::Done => panic!("query returned no rows: {sql}"),
            StepResult::Interrupt | StepResult::Busy => panic!("query interrupted or busy: {sql}"),
        }
    }
}
