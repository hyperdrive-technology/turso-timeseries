#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::model::MetricPoint;

use super::batch::IngestBatch;

/// In-memory write buffer grouping points by hypertable.
#[derive(Debug, Default)]
pub struct WriteBuffer {
    points: Vec<MetricPoint>,
}

impl WriteBuffer {
  #[must_use]
  pub fn new() -> Self {
      Self::default()
  }

    pub fn push(&mut self, point: MetricPoint) {
        self.points.push(point);
    }

    pub fn drain_batch(&mut self) -> IngestBatch {
        IngestBatch::new(core::mem::take(&mut self.points))
    }
}
