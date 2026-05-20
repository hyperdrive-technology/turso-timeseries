//! PLAN-v2 catalog table names (see `PLAN-v2.md` §9).

pub const TABLE_SCHEMA_VERSION: &str = "_tts_schema_version";
pub const TABLE_HYPERTABLES: &str = "_tts_hypertables";
pub const TABLE_SERIES: &str = "_tts_series";
pub const TABLE_CHUNKS: &str = "_tts_chunks";
pub const TABLE_SEGMENTS: &str = "_tts_segments";
pub const TABLE_SEGMENT_COLUMNS: &str = "_tts_segment_columns";
pub const TABLE_INVALIDATIONS: &str = "_tts_invalidations";
pub const TABLE_ROLLUP_POLICIES: &str = "_tts_rollup_policies";
pub const TABLE_HYPERTABLE_STATS: &str = "_tts_hypertable_stats";
pub const TABLE_SERIES_STATS: &str = "_tts_series_stats";
pub const TABLE_MAINTENANCE_JOBS: &str = "_tts_maintenance_jobs";
pub const TABLE_TAG_INDEX: &str = "_tts_tag_index";
