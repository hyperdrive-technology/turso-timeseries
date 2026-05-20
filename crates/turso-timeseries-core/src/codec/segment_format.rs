#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::error::{Error, Result};
use crate::model::{SEGMENT_MAGIC, SEGMENT_VERSION};
use crate::model::{ChunkId, TimestampMicros};
use crate::query::EncodedPointRow;

use super::{decode_f64_xor_column, decode_i64_delta_column, encode_f64_xor_column, encode_i64_delta_column};

/// One encoded column block inside a segment.
#[derive(Debug, Clone, PartialEq)]
pub struct SegmentColumn {
    pub name: &'static str,
    pub data: Vec<u8>,
}

/// Encode points into TTS segment v1 bytes.
pub fn encode_segment_v1(
    chunk_id: ChunkId,
    times: &[i64],
    values: &[f64],
    qualities: &[i32],
) -> Result<Vec<u8>> {
    if times.len() != values.len() || times.len() != qualities.len() {
        return Err(Error::MalformedLineProtocol {
            line: 0,
            detail: "column length mismatch",
        });
    }

    let min_time = times.first().copied().unwrap_or(0);
    let max_time = times.last().copied().unwrap_or(0);
    let row_count = times.len() as u64;

    let time_col = encode_i64_delta_column(times);
    let value_col = encode_f64_xor_column(values);
    let quality_col = encode_i32_le_column(qualities);

    let mut payload = Vec::new();
    payload.extend_from_slice(SEGMENT_MAGIC);
    payload.push(SEGMENT_VERSION);
    payload.extend_from_slice(&0u16.to_le_bytes()); // flags
    payload.extend_from_slice(&0u32.to_le_bytes()); // schema_hash
    payload.extend_from_slice(&chunk_id.0.to_le_bytes());
    payload.extend_from_slice(&min_time.to_le_bytes());
    payload.extend_from_slice(&max_time.to_le_bytes());
    payload.extend_from_slice(&row_count.to_le_bytes());

    for col in [("time", time_col), ("value", value_col), ("quality", quality_col)] {
        payload.extend_from_slice(col.0.as_bytes());
        payload.push(0);
        payload.extend_from_slice(&(col.1.len() as u32).to_le_bytes());
        payload.extend_from_slice(&col.1);
    }

    let checksum = crc32(payload.as_slice());
    payload.extend_from_slice(&checksum.to_le_bytes());
    Ok(payload)
}

/// Decode TTS segment v1 bytes into point rows.
pub fn decode_segment_v1(payload: &[u8]) -> Result<Vec<EncodedPointRow>> {
    if payload.len() < SEGMENT_MAGIC.len() + 1 {
        return Err(Error::InvalidSegmentMagic);
    }
    if &payload[..SEGMENT_MAGIC.len()] != SEGMENT_MAGIC {
        return Err(Error::InvalidSegmentMagic);
    }
    if payload[SEGMENT_MAGIC.len()] != SEGMENT_VERSION {
        return Err(Error::InvalidSegmentVersion {
            found: payload[SEGMENT_MAGIC.len()],
        });
    }

    let header_end = SEGMENT_MAGIC.len() + 1 + 2 + 4 + 8 + 8 + 8 + 8;
    if payload.len() < header_end {
        return Err(Error::InvalidSegmentMagic);
    }

    let mut offset = SEGMENT_MAGIC.len() + 1 + 2 + 4;
    let _chunk_id = i64::from_le_bytes(payload[offset..offset + 8].try_into().unwrap());
    offset += 8;
    let min_time = i64::from_le_bytes(payload[offset..offset + 8].try_into().unwrap());
    offset += 8;
    let _max_time = i64::from_le_bytes(payload[offset..offset + 8].try_into().unwrap());
    offset += 8;
    let row_count = u64::from_le_bytes(payload[offset..offset + 8].try_into().unwrap()) as usize;
    offset += 8;

    let mut time_bytes = None;
    let mut value_bytes = None;
    let mut quality_bytes = None;

    while offset + 5 < payload.len() {
        let name_end = payload[offset..]
            .iter()
            .position(|&b| b == 0)
            .map(|p| offset + p);
        let Some(name_end) = name_end else {
            break;
        };
        let name = core::str::from_utf8(&payload[offset..name_end]).unwrap_or("");
        offset = name_end + 1;
        if offset + 4 > payload.len() {
            break;
        }
        let len = u32::from_le_bytes(payload[offset..offset + 4].try_into().unwrap()) as usize;
        offset += 4;
        if offset + len > payload.len() {
            break;
        }
        let data = payload[offset..offset + len].to_vec();
        offset += len;
        match name {
            "time" => time_bytes = Some(data),
            "value" => value_bytes = Some(data),
            "quality" => quality_bytes = Some(data),
            _ => {}
        }
    }

    let times = decode_i64_delta_column(time_bytes.as_deref().unwrap_or(&[]))?;
    let values = decode_f64_xor_column(value_bytes.as_deref().unwrap_or(&[]))?;
    let qualities = decode_i32_le_column(quality_bytes.as_deref().unwrap_or(&[]))?;

    let mut rows = Vec::new();
    for i in 0..row_count.min(times.len()).min(values.len()) {
        rows.push(EncodedPointRow {
            time: TimestampMicros(times.get(i).copied().unwrap_or(min_time)),
            value: *values.get(i).unwrap_or(&0.0),
            quality: *qualities.get(i).unwrap_or(&0),
        });
    }
    Ok(rows)
}

fn encode_i32_le_column(values: &[i32]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(values.len() as u32).to_le_bytes());
    for value in values {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

fn decode_i32_le_column(bytes: &[u8]) -> Result<Vec<i32>> {
    if bytes.len() < 4 {
        return Ok(Vec::new());
    }
    let count = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
    let mut out = Vec::with_capacity(count);
    let mut offset = 4;
    for _ in 0..count {
        if offset + 4 > bytes.len() {
            break;
        }
        out.push(i32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()));
        offset += 4;
    }
    Ok(out)
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_roundtrip() {
        let times = vec![1_000, 2_000, 3_500];
        let values = vec![10.0, 11.5, 9.25];
        let qualities = vec![0, 1, 0];
        let encoded = encode_segment_v1(ChunkId(1), &times, &values, &qualities).unwrap();
        let decoded = decode_segment_v1(&encoded).unwrap();
        assert_eq!(decoded.len(), 3);
        assert_eq!(decoded[0].time.0, 1_000);
        assert_eq!(decoded[1].value, 11.5);
    }
}
