#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

/// Gorilla-style XOR encoding for f64 columns (v0 stores XOR deltas of IEEE bits).
pub fn encode_f64_xor_column(values: &[f64]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(values.len() as u32).to_le_bytes());
    let mut prev_bits = 0u64;
    for value in values {
        let bits = value.to_bits();
        let xor = bits ^ prev_bits;
        out.extend_from_slice(&xor.to_le_bytes());
        prev_bits = bits;
    }
    out
}

pub fn decode_f64_xor_column(bytes: &[u8]) -> crate::Result<Vec<f64>> {
    if bytes.len() < 4 {
        return Ok(Vec::new());
    }
    let count = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
    let mut out = Vec::with_capacity(count);
    let mut offset = 4;
    let mut prev_bits = 0u64;
    for _ in 0..count {
        if offset + 8 > bytes.len() {
            break;
        }
        let xor = u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
        offset += 8;
        prev_bits ^= xor;
        out.push(f64::from_bits(prev_bits));
    }
    Ok(out)
}
