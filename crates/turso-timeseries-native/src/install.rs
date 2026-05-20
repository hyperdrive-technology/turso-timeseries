use turso_timeseries_catalog::MIGRATIONS;

/// Apply all catalog migrations idempotently.
pub async fn install(conn: &turso::Connection) -> turso::Result<()> {
    conn.execute("PRAGMA foreign_keys = ON", ()).await?;
    for migration in MIGRATIONS {
        conn.execute_batch(migration.sql).await?;
    }
    Ok(())
}
