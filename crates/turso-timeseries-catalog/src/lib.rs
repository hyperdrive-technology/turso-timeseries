//! Embedded SQL migrations for turso-timeseries catalog tables.

pub mod migrations;
pub mod schema;
pub mod views;

pub use migrations::{apply_migration_sql, MigrationStep, MIGRATIONS, SCHEMA_VERSION};
