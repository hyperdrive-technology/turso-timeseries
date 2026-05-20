#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use super::schema::FieldColumnDef;

/// Opaque hypertable identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct HypertableId(pub i64);

/// Chunk interval in microseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DurationMicros(pub i64);

/// Hypertable catalog metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hypertable {
    pub id: HypertableId,
    pub name: String,
    pub time_column: String,
    pub tag_columns: Vec<String>,
    pub field_columns: Vec<FieldColumnDef>,
    pub chunk_interval: DurationMicros,
}

/// Backward-compatible alias from PLAN-v2 naming.
pub type FieldColumn = FieldColumnDef;
