use crate::model::ChunkId;

/// Segment blob persistence trait.
pub trait SegmentStore {
    fn insert_segment(&mut self, chunk_id: ChunkId, payload: &[u8]) -> crate::Result<i64>;
}
