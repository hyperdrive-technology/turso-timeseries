#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::model::MetricPoint;

/// Batch of points ready for encoding and catalog writes.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct IngestBatch {
    pub points: Vec<MetricPoint>,
}

impl IngestBatch {
    #[must_use]
    pub fn new(points: Vec<MetricPoint>) -> Self {
        Self { points }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }
}
