//! Embedded SQL migrations for the conservative row layout (`PLAN.md` §11.1, Phase 1).
//!
//! Application code applies these in order on a Turso/SQLite connection (with
//! `PRAGMA foreign_keys = ON`). This crate does not depend on the `turso` crate.

/// Schema/migration bundle version tracked in `_tts_schema_version` after all steps apply.
pub const SCHEMA_VERSION: u32 = 4;

/// Migration step `0001_*`.
pub const MIGRATION_0001_VERSION: u32 = 1;

/// Migration step `0002_*`.
pub const MIGRATION_0002_VERSION: u32 = 2;

/// Migration step `0003_*`.
pub const MIGRATION_0003_VERSION: u32 = 3;

/// Migration step `0004_*`.
pub const MIGRATION_0004_VERSION: u32 = 4;

/// One ordered migration: monotonic `version`, stable `id`, full SQL text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MigrationStep {
    /// Logical migration number (matches filename prefix and `_tts_schema_version` floor).
    pub version: u32,
    /// Stable identifier (filename stem without extension).
    pub id: &'static str,
    /// Full SQL script for this step.
    pub sql: &'static str,
}

/// Ordered Phase 1 migrations (lowest `version` first).
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
];

/// Collect `CREATE TABLE IF NOT EXISTS <name>` identifiers (Turso/SQLite DDL style).
/// Zero extra deps; assumes one statement start per line as in shipped migrations.
#[must_use]
pub fn scan_create_table_names(sql: &str) -> Vec<String> {
    const PREFIX: &str = "CREATE TABLE IF NOT EXISTS ";
    let mut out = Vec::new();
    for line in sql.lines() {
        let trimmed = line.trim_start();
        let upper = trimmed
            .as_bytes()
            .iter()
            .map(|b| b.to_ascii_uppercase())
            .collect::<Vec<_>>();
        let upper = String::from_utf8_lossy(&upper);
        if let Some(idx) = upper.find(PREFIX) {
            let rest = &trimmed[idx + PREFIX.len()..];
            let name = rest
                .split(|c: char| c.is_whitespace() || c == '(')
                .next()
                .unwrap_or("")
                .trim_matches('`');
            if !name.is_empty() {
                out.push(name.to_string());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_nonempty() {
        for m in MIGRATIONS {
            assert!(
                !m.sql.trim().is_empty(),
                "migration {} SQL must not be empty",
                m.id
            );
        }
    }

    #[test]
    fn migrations_ordered_unique_versions() {
        let mut prev: Option<u32> = None;
        let mut seen = std::collections::BTreeSet::new();
        for m in MIGRATIONS {
            assert!(
                seen.insert(m.version),
                "duplicate migration version {}",
                m.version
            );
            if let Some(p) = prev {
                assert!(
                    p < m.version,
                    "migrations must be strictly ascending by version: {} then {}",
                    p,
                    m.version
                );
            }
            prev = Some(m.version);
        }
        assert_eq!(
            prev,
            Some(SCHEMA_VERSION),
            "SCHEMA_VERSION must match last migration version"
        );
    }

    #[test]
    fn migration_ids_match_filenames() {
        assert_eq!(MIGRATIONS[0].id, "0001_catalog_and_samples");
        assert_eq!(MIGRATIONS[1].id, "0002_policies_and_jobs");
        assert_eq!(MIGRATIONS[2].id, "0003_columnar_hypertables");
        assert_eq!(MIGRATIONS[3].id, "0004_materialized_rollups");
    }

    #[test]
    fn sql_defines_expected_tables() {
        let combined: String = MIGRATIONS
            .iter()
            .map(|m| m.sql)
            .collect::<Vec<_>>()
            .concat();
        for name in [
            "_tts_schema_version",
            "_tts_series",
            "_tts_samples",
            "_tts_retention_policies",
            "_tts_rollup_policies",
            "_tts_rollup_watermarks",
            "_tts_invalidations",
            "_tts_jobs",
            "_tts_hypertables",
            "_tts_chunks",
            "_tts_segments",
            "_tts_segment_columns",
            "_tts_rollups",
        ] {
            assert!(
                combined.contains(name),
                "expected table token {name} in migration SQL"
            );
        }
    }

    #[test]
    fn migration_versions_contiguous_from_one() {
        for (i, m) in MIGRATIONS.iter().enumerate() {
            assert_eq!(
                m.version,
                (i + 1) as u32,
                "migrations must be contiguous 1..=n (got {} at index {})",
                m.version,
                i
            );
        }
    }

    #[test]
    fn public_migration_version_constants_align_with_slice() {
        assert_eq!(MIGRATION_0001_VERSION, MIGRATIONS[0].version);
        assert_eq!(MIGRATION_0002_VERSION, MIGRATIONS[1].version);
        assert_eq!(MIGRATION_0003_VERSION, MIGRATIONS[2].version);
        assert_eq!(MIGRATION_0004_VERSION, MIGRATIONS[3].version);
        assert_eq!(MIGRATIONS.last().unwrap().version, SCHEMA_VERSION);
    }

    #[test]
    fn each_migration_updates_schema_floor_to_its_version() {
        for m in MIGRATIONS {
            let needle = format!("MAX(version, {})", m.version);
            assert!(
                m.sql.contains(&needle),
                "migration {} must bump _tts_schema_version with {}",
                m.id,
                needle
            );
        }
    }

    #[test]
    fn create_table_names_match_expected_catalog() {
        let mut from_sql = std::collections::BTreeSet::new();
        for m in MIGRATIONS {
            for n in scan_create_table_names(m.sql) {
                assert!(
                    from_sql.insert(n.clone()),
                    "duplicate CREATE TABLE name {n} in {}",
                    m.id
                );
            }
        }
        let expected = [
            "_tts_schema_version",
            "_tts_series",
            "_tts_samples",
            "_tts_retention_policies",
            "_tts_rollup_policies",
            "_tts_rollup_watermarks",
            "_tts_invalidations",
            "_tts_jobs",
            "_tts_hypertables",
            "_tts_chunks",
            "_tts_segments",
            "_tts_segment_columns",
            "_tts_rollups",
        ];
        let expected_set: std::collections::BTreeSet<String> =
            expected.iter().map(|s| (*s).to_string()).collect();
        assert_eq!(
            from_sql, expected_set,
            "CREATE TABLE names must match Phase 1 catalog exactly"
        );
    }

    #[test]
    fn migration_ids_use_four_digit_prefix_matching_version() {
        for m in MIGRATIONS {
            let prefix = format!("{:04}", m.version);
            assert!(
                m.id.starts_with(&prefix),
                "migration id {:?} should start with {} to match version {}",
                m.id,
                prefix,
                m.version
            );
        }
    }
}
