#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use super::ids::{FieldId, SeriesId};
use super::value::FieldValue;

/// Storage-oriented point keyed by resolved `series_id` (recommended write path).
#[derive(Debug, Clone, PartialEq)]
pub struct Point {
    pub series_id: SeriesId,
    pub time: i64,
    pub field: FieldId,
    pub value: FieldValue,
}

/// Encoded ingest batch (`TTS_BATCH_V1`) for `tts_write_batch`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedBatch {
    pub bytes: Vec<u8>,
    pub min_time: i64,
    pub max_time: i64,
    pub row_count: u64,
}

impl EncodedBatch {
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}
