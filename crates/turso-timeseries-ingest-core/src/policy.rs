/// When to seal an ingest buffer into an `EncodedBatch`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlushPolicy {
    pub max_points: usize,
    pub max_bytes: usize,
    pub max_age_ms: u64,
}

impl Default for FlushPolicy {
    fn default() -> Self {
        Self {
            max_points: 10_000,
            max_bytes: 1_000_000,
            max_age_ms: 250,
        }
    }
}
