#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

/// Typed field value for time-series points.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldValue {
    Null,
    I64(i64),
    F64(f64),
    Bool(bool),
    Text(String),
    Blob(Vec<u8>),
}

impl FieldValue {
    /// Returns true when the value can participate in numeric aggregates.
    #[must_use]
    pub fn is_numeric(&self) -> bool {
        matches!(self, Self::I64(_) | Self::F64(_))
    }

    /// Coerce to `f64` for aggregate state machines.
    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::I64(v) => Some(*v as f64),
            Self::F64(v) if v.is_finite() => Some(*v),
            _ => None,
        }
    }
}
