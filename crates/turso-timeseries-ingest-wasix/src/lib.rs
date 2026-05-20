//! WASIX compatibility ingest — experimental; not loaded inside the Turso extension.
//!
//! Host should provide `write_batch(hypertable, bytes)` or a network sink.

use turso_timeseries_ingest_core::{FlushPolicy, IngestBuffer};

/// Guest-side ingest state for Mode A (hostcall sink).
pub struct WasixIngestGuest {
    pub hypertable: String,
    buffer: IngestBuffer,
}

impl WasixIngestGuest {
    #[must_use]
    pub fn new(hypertable: impl Into<String>, policy: FlushPolicy) -> Self {
        Self {
            hypertable: hypertable.into(),
            buffer: IngestBuffer::new(policy),
        }
    }

    pub fn push_frame(&mut self, frame: &[u8], now_ms: u64) -> turso_timeseries_ingest_core::Result<Option<Vec<turso_timeseries_ingest_core::MetricPoint>>> {
        self.buffer.push_frame(frame, now_ms)
    }
}
