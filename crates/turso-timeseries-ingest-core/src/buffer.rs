use turso_timeseries_core::ingest::parse_line_protocol_batch;
use turso_timeseries_core::model::MetricPoint;

use crate::error::{IngestError, Result};
use crate::policy::FlushPolicy;

/// Buffers line-protocol or raw frames until flush policy triggers.
#[derive(Debug, Default)]
pub struct IngestBuffer {
    points: Vec<MetricPoint>,
    estimated_bytes: usize,
    policy: FlushPolicy,
    opened_at_ms: u64,
}

impl IngestBuffer {
    #[must_use]
    pub fn new(policy: FlushPolicy) -> Self {
        Self {
            points: Vec::new(),
            estimated_bytes: 0,
            policy,
            opened_at_ms: 0,
        }
    }

    /// Push UTF-8 line protocol text; returns sealed batch when policy triggers.
    pub fn push_lp_text(&mut self, text: &str, now_ms: u64) -> Result<Option<Vec<MetricPoint>>> {
        if self.opened_at_ms == 0 {
            self.opened_at_ms = now_ms;
        }
        let parsed = parse_line_protocol_batch(text)?;
        self.estimated_bytes += text.len();
        self.points.extend(parsed);
        if self.should_flush(now_ms) {
            return Ok(Some(self.seal()?));
        }
        Ok(None)
    }

    /// Push opaque frame bytes interpreted as UTF-8 line protocol.
    pub fn push_frame(&mut self, frame: &[u8], now_ms: u64) -> Result<Option<Vec<MetricPoint>>> {
        let text = core::str::from_utf8(frame).map_err(|_| IngestError::FrameTooLarge)?;
        self.push_lp_text(text, now_ms)
    }

    fn should_flush(&self, now_ms: u64) -> bool {
        self.points.len() >= self.policy.max_points
            || self.estimated_bytes >= self.policy.max_bytes
            || now_ms.saturating_sub(self.opened_at_ms) >= self.policy.max_age_ms
    }

    fn seal(&mut self) -> Result<Vec<MetricPoint>> {
        if self.points.is_empty() {
            return Err(IngestError::BufferEmpty);
        }
        self.estimated_bytes = 0;
        self.opened_at_ms = 0;
        Ok(core::mem::take(&mut self.points))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flushes_by_point_count() {
        let mut buf = IngestBuffer::new(FlushPolicy {
            max_points: 2,
            max_bytes: usize::MAX,
            max_age_ms: u64::MAX,
        });
        let out = buf
            .push_lp_text(
                "m,t=a v=1 1778000000000000000\nm,t=a v=2 1778000000000000001",
                0,
            )
            .unwrap();
        assert!(out.is_some());
        assert_eq!(out.unwrap().len(), 2);
    }
}
