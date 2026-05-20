/// Logical write transaction boundary for MVCC-safe ingest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WriteTransaction {
    pub id: u64,
}

impl WriteTransaction {
    #[must_use]
    pub const fn new(id: u64) -> Self {
        Self { id }
    }
}
