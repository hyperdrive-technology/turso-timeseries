#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use super::{FieldValue, TimestampMicros};

/// One measurement sample in Influx line-protocol shape.
#[derive(Debug, Clone, PartialEq)]
pub struct MetricPoint {
    pub table: String,
    pub time: TimestampMicros,
    pub tags: Vec<(String, String)>,
    pub fields: Vec<(String, FieldValue)>,
}

impl MetricPoint {
    /// First field value, if any.
    #[must_use]
    pub fn first_field_value(&self) -> Option<&FieldValue> {
        self.fields.first().map(|(_, v)| v)
    }
}
