#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use super::{ChunkId, TimestampMicros};

pub const SEGMENT_MAGIC: &[u8; 12] = b"TTS_SEGMENT_";
pub const SEGMENT_VERSION: u8 = 1;

/// Decoded segment header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentHeader {
    pub chunk_id: ChunkId,
    pub min_time: TimestampMicros,
    pub max_time: TimestampMicros,
    pub row_count: u64,
    pub schema_hash: u32,
}

/// In-memory segment before/after encoding.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub header: SegmentHeader,
    pub payload: Vec<u8>,
}
