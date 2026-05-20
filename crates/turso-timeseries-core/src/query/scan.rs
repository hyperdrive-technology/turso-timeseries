#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::codec::segment_format::decode_segment_v1;
use crate::model::TimestampMicros;

/// One decoded row from a segment payload.
#[derive(Debug, Clone, PartialEq)]
pub struct EncodedPointRow {
    pub time: TimestampMicros,
    pub value: f64,
    pub quality: i32,
}

/// Decode segment bytes into point rows.
pub fn decode_segment_points(payload: &[u8]) -> crate::Result<Vec<EncodedPointRow>> {
    decode_segment_v1(payload)
}
