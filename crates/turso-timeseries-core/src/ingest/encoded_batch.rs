#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::error::Result;
use crate::model::{EncodedBatch, Point};

pub const BATCH_MAGIC: &[u8; 12] = b"TTS_BATCH_V1";

/// Encode resolved points into `TTS_BATCH_V1` for extension `tts_write_batch`.
pub fn encode_batch(points: &[Point]) -> Result<EncodedBatch> {
    if points.is_empty() {
        return Ok(EncodedBatch {
            bytes: BATCH_MAGIC.to_vec(),
            min_time: 0,
            max_time: 0,
            row_count: 0,
        });
    }

    let min_time = points.iter().map(|p| p.time).min().unwrap_or(0);
    let max_time = points.iter().map(|p| p.time).max().unwrap_or(0);
    let row_count = points.len() as u64;

    let mut bytes = Vec::from(BATCH_MAGIC);
    bytes.extend_from_slice(&row_count.to_le_bytes());
    bytes.extend_from_slice(&min_time.to_le_bytes());
    bytes.extend_from_slice(&max_time.to_le_bytes());

    for point in points {
        bytes.extend_from_slice(&point.series_id.0.to_le_bytes());
        bytes.extend_from_slice(&point.time.to_le_bytes());
        bytes.extend_from_slice(&point.field.0.to_le_bytes());
        encode_value(&mut bytes, &point.value)?;
    }

    Ok(EncodedBatch {
        bytes,
        min_time,
        max_time,
        row_count,
    })
}

fn encode_value(out: &mut Vec<u8>, value: &crate::model::FieldValue) -> Result<()> {
    use crate::model::FieldValue;
    match value {
        FieldValue::Null => out.push(0),
        FieldValue::I64(v) => {
            out.push(1);
            out.extend_from_slice(&v.to_le_bytes());
        }
        FieldValue::F64(v) => {
            out.push(2);
            out.extend_from_slice(&v.to_le_bytes());
        }
        FieldValue::Bool(v) => {
            out.push(3);
            out.push(u8::from(*v));
        }
        FieldValue::Text(s) => {
            out.push(4);
            let b = s.as_bytes();
            out.extend_from_slice(&(b.len() as u32).to_le_bytes());
            out.extend_from_slice(b);
        }
        FieldValue::Blob(b) => {
            out.push(5);
            out.extend_from_slice(&(b.len() as u32).to_le_bytes());
            out.extend_from_slice(b);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{FieldId, SeriesId};
    use crate::model::FieldValue;

    #[test]
    fn batch_has_magic_header() {
        let batch = encode_batch(&[Point {
            series_id: SeriesId(1),
            time: 100,
            field: FieldId(0),
            value: FieldValue::F64(1.0),
        }])
        .unwrap();
        assert!(batch.bytes.starts_with(BATCH_MAGIC));
        assert_eq!(batch.row_count, 1);
    }
}
