use crate::model::TimestampMicros;

/// Report from retention application.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RetentionReport {
    pub chunks_deleted: u64,
}

/// Decide whether a chunk is fully expired.
#[must_use]
pub fn chunk_expired(chunk_end: TimestampMicros, cutoff: TimestampMicros) -> bool {
    chunk_end.0 <= cutoff.0
}

/// Count chunks that would be deleted by retention.
pub fn apply_retention(chunk_ends: &[TimestampMicros], cutoff: TimestampMicros) -> RetentionReport {
    let chunks_deleted = chunk_ends
        .iter()
        .filter(|end| chunk_expired(**end, cutoff))
        .count() as u64;
    RetentionReport { chunks_deleted }
}
