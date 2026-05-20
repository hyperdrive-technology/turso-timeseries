//! Native Turso adapter for the dependency-free planning API.
//!
//! This module is intentionally thin: catalog design, series identity and SQL
//! planning stay in the core crate surface, while this adapter only translates
//! bind values and executes statements against the current `turso` crate API.

use turso_timeseries_catalog::MIGRATIONS;
use crate::{SqlBatch, SqlStatement, SqlValue};

/// Apply all embedded migrations to a native Turso connection.
///
/// Callers should apply this before executing ingest or policy statements. The
/// migration SQL is idempotent, so running this more than once is expected.
pub async fn apply_migrations(conn: &turso::Connection) -> turso::Result<()> {
    conn.execute("PRAGMA foreign_keys = ON", ()).await?;
    for migration in MIGRATIONS {
        conn.execute_batch(migration.sql).await?;
    }
    Ok(())
}

/// Execute one planned statement against a native Turso connection.
pub async fn execute_statement(
    conn: &turso::Connection,
    statement: &SqlStatement,
) -> turso::Result<u64> {
    let params = statement
        .params
        .iter()
        .cloned()
        .map(turso_value)
        .collect::<Vec<_>>();
    conn.execute(statement.sql, turso::params_from_iter(params))
        .await
}

/// Execute a planned batch against a native Turso connection.
///
/// The caller owns transaction boundaries. For ingest, prefer wrapping this in
/// `BEGIN`/`COMMIT` at the application layer until the adapter grows an explicit
/// transaction helper.
pub async fn execute_batch(conn: &turso::Connection, batch: &SqlBatch) -> turso::Result<u64> {
    let mut changed = 0;
    for statement in &batch.statements {
        changed += execute_statement(conn, statement).await?;
    }
    Ok(changed)
}

fn turso_value(value: SqlValue) -> turso::Value {
    match value {
        SqlValue::Null => turso::Value::Null,
        SqlValue::Integer(value) => turso::Value::Integer(value),
        SqlValue::Real(value) => turso::Value::Real(value),
        SqlValue::Text(value) => turso::Value::Text(value),
        SqlValue::Blob(value) => turso::Value::Blob(value),
    }
}
