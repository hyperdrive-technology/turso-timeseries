//! Native Turso integration: install, ingest, query, and maintenance.

mod connection;
mod install;
mod maintenance;
mod query;
mod writer;

pub use install::install;
pub use maintenance::{run_maintenance, MaintenanceOptions, MaintenanceReport};
pub use query::{read_points, scan_raw_rows, RawPointRow};
pub use writer::{create_hypertable, write_line_protocol, write_points, WriteStats};

/// Primary native API surface (`PLAN-v2.md` §12).
pub struct Timeseries;

impl Timeseries {
    pub async fn install(conn: &turso::Connection) -> turso::Result<()> {
        install(conn).await
    }

    pub async fn create_hypertable(
        conn: &turso::Connection,
        table: &str,
        time_column: &str,
        chunk_interval_micros: i64,
    ) -> turso::Result<i64> {
        create_hypertable(conn, table, time_column, chunk_interval_micros).await
    }

    pub async fn write_line_protocol(
        conn: &turso::Connection,
        line: &str,
    ) -> turso::Result<WriteStats> {
        write_line_protocol(conn, line).await
    }

    pub async fn write_points(
        conn: &turso::Connection,
        table: &str,
        lines: &str,
    ) -> turso::Result<WriteStats> {
        write_points(conn, table, lines).await
    }

    pub async fn run_maintenance(
        conn: &turso::Connection,
        options: MaintenanceOptions,
    ) -> turso::Result<MaintenanceReport> {
        run_maintenance(conn, options).await
    }

    pub async fn read_points(
        conn: &turso::Connection,
        table: &str,
        from_micros: Option<i64>,
        to_micros: Option<i64>,
    ) -> turso::Result<Vec<RawPointRow>> {
        read_points(conn, table, from_micros, to_micros).await
    }
}
