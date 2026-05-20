//! Ordered migrations: legacy row/columnar layout plus PLAN-v2 catalog extensions.

/// Schema version after all migrations apply.
pub const SCHEMA_VERSION: u32 = 5;

pub const MIGRATION_0001_VERSION: u32 = 1;
pub const MIGRATION_0002_VERSION: u32 = 2;
pub const MIGRATION_0003_VERSION: u32 = 3;
pub const MIGRATION_0004_VERSION: u32 = 4;
pub const MIGRATION_0005_VERSION: u32 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MigrationStep {
    pub version: u32,
    pub id: &'static str,
    pub sql: &'static str,
}

pub static MIGRATIONS: &[MigrationStep] = &[
    MigrationStep {
        version: MIGRATION_0001_VERSION,
        id: "0001_catalog_and_samples",
        sql: include_str!("../migrations/0001_catalog_and_samples.sql"),
    },
    MigrationStep {
        version: MIGRATION_0002_VERSION,
        id: "0002_policies_and_jobs",
        sql: include_str!("../migrations/0002_policies_and_jobs.sql"),
    },
    MigrationStep {
        version: MIGRATION_0003_VERSION,
        id: "0003_columnar_hypertables",
        sql: include_str!("../migrations/0003_columnar_hypertables.sql"),
    },
    MigrationStep {
        version: MIGRATION_0004_VERSION,
        id: "0004_materialized_rollups",
        sql: include_str!("../migrations/0004_materialized_rollups.sql"),
    },
    MigrationStep {
        version: MIGRATION_0005_VERSION,
        id: "0005_plan_v2_catalog",
        sql: include_str!("../migrations/0005_plan_v2_catalog.sql"),
    },
];

/// Concatenate all migration SQL for tools that need a single script.
#[must_use]
pub fn apply_migration_sql() -> String {
    MIGRATIONS
        .iter()
        .map(|step| step.sql)
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_are_monotonic() {
        let mut prev = 0;
        for step in MIGRATIONS {
            assert!(step.version > prev);
            prev = step.version;
        }
        assert_eq!(prev, SCHEMA_VERSION);
    }
}
