//! Native ingest service — sockets and timers live here, not in the extension.

use turso_timeseries_ingest_core::{FlushPolicy, IngestBuffer};

/// High-throughput native ingest bound to one hypertable name.
#[derive(Debug, Clone)]
pub struct NativeIngestService {
    pub hypertable: String,
    pub policy: FlushPolicy,
}

impl NativeIngestService {
    #[must_use]
    pub fn new(hypertable: impl Into<String>, policy: FlushPolicy) -> Self {
        Self {
            hypertable: hypertable.into(),
            policy,
        }
    }

    /// Create a fresh buffer for one connection/stream handler.
    #[must_use]
    pub fn buffer(&self) -> IngestBuffer {
        IngestBuffer::new(self.policy)
    }

    /// Hypertable name passed to `tts_write_batch` at the SQL boundary.
    #[must_use]
    pub fn hypertable_name(&self) -> &str {
        &self.hypertable
    }
}
