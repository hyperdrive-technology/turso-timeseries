#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

/// Delta + varint-style encoding for i64 timestamps (v0 uses fixed delta stream).
pub fn encode_i64_delta_column(values: &[i64]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(values.len() as u32).to_le_bytes());
    let mut prev = 0i64;
    for value in values {
        let delta = value - prev;
        out.extend_from_slice(&delta.to_le_bytes());
        prev = *value;
    }
    out
}

pub fn decode_i64_delta_column(bytes: &[u8]) -> crate::Result<Vec<i64>> {
    if bytes.len() < 4 {
        return Ok(Vec::new());
    }
    let count = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
    let mut out = Vec::with_capacity(count);
    let mut offset = 4;
    let mut prev = 0i64;
    for _ in 0..count {
        if offset + 8 > bytes.len() {
            break;
        }
        let delta = i64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
        offset += 8;
        prev += delta;
        out.push(prev);
    }
    Ok(out)
}
