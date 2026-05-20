use super::{DurationMicros, HypertableId, TimestampMicros};

/// Opaque chunk identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ChunkId(pub i64);

/// Chunk-level statistics for pruning and cost estimation.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ChunkStats {
    pub min_time: Option<TimestampMicros>,
    pub max_time: Option<TimestampMicros>,
    pub row_count: u64,
}

/// Chunk metadata row.
#[derive(Debug, Clone, PartialEq)]
pub struct ChunkMeta {
    pub id: ChunkId,
    pub hypertable_id: HypertableId,
    pub time_start: TimestampMicros,
    pub time_end: TimestampMicros,
    pub sealed: bool,
    pub level: u32,
    pub row_count: u64,
    pub stats: ChunkStats,
}

impl ChunkMeta {
    #[must_use]
    pub fn width_micros(&self) -> i64 {
        self.time_end.0 - self.time_start.0
    }
}
