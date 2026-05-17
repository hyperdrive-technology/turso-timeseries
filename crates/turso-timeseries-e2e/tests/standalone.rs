use std::process::Command;
use std::time::{Duration, Instant};

use std::sync::Arc;
use turso_core::{Database, DatabaseOpts, MemoryIO, OpenFlags, StepResult, IO};

#[test]
fn standalone_binary_loads_extension_and_round_trips_rows() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_tts-standalone-e2e"))
        .spawn()
        .expect("spawn standalone E2E binary");
    let started = Instant::now();

    loop {
        if let Some(status) = child.try_wait().expect("poll standalone E2E binary") {
            assert!(status.success(), "standalone E2E binary failed: {status}");
            return;
        }
        if started.elapsed() > Duration::from_secs(30) {
            child.kill().ok();
            panic!("standalone E2E binary timed out");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn memory_turso_smoke_round_trip_without_filesystem() {
    let io: Arc<dyn IO> = Arc::new(MemoryIO::new());
    let db = Database::open_file_with_flags(
        io.clone(),
        ":memory:",
        OpenFlags::Create,
        DatabaseOpts::new().turso_cli(),
        None,
    )
    .expect("open memory Turso DB");
    let conn = db.connect().expect("connect");

    execute(
        io.as_ref(),
        &conn,
        "CREATE TABLE e2e_smoke (id INTEGER PRIMARY KEY, value TEXT)",
    );
    execute(
        io.as_ref(),
        &conn,
        "INSERT INTO e2e_smoke (value) VALUES ('ok')",
    );
    assert_eq!(
        query_one_text(
            io.as_ref(),
            &conn,
            "SELECT value FROM e2e_smoke WHERE id = 1"
        ),
        "ok"
    );
}

fn execute(io: &dyn IO, conn: &Arc<turso_core::Connection>, sql: &str) {
    let Some(mut stmt) = conn.query(sql).expect(sql) else {
        return;
    };
    loop {
        match stmt.step().expect("step") {
            StepResult::Done => return,
            StepResult::Row => {}
            StepResult::IO => io.step().expect("run IO"),
            StepResult::Interrupt | StepResult::Busy => panic!("statement interrupted or busy"),
        }
    }
}

fn query_one_text(io: &dyn IO, conn: &Arc<turso_core::Connection>, sql: &str) -> String {
    let mut stmt = conn.prepare(sql).expect(sql);
    loop {
        match stmt.step().expect(sql) {
            StepResult::Row => return stmt.row().expect("row").get::<String>(0).unwrap(),
            StepResult::IO => io.step().expect("run IO"),
            StepResult::Done => panic!("query returned no rows: {sql}"),
            StepResult::Interrupt | StepResult::Busy => panic!("query interrupted or busy: {sql}"),
        }
    }
}
