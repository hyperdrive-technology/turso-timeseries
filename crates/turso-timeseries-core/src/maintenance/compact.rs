#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

/// Compaction merges adjacent segment payloads when row counts are small.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CompactionReport {
    pub segments_in: u64,
    pub segments_out: u64,
}

pub fn compact_segments(segments: &[Vec<u8>], max_rows_per_segment: usize) -> (Vec<Vec<u8>>, CompactionReport) {
    if segments.is_empty() {
        return (Vec::new(), CompactionReport::default());
    }
    let mut out = Vec::new();
    let mut current = segments[0].clone();
    for segment in segments.iter().skip(1) {
        if current.len() + segment.len() <= max_rows_per_segment * 32 {
            current.extend_from_slice(segment);
        } else {
            out.push(core::mem::take(&mut current));
            current = segment.clone();
        }
    }
    out.push(current);
    let report = CompactionReport {
        segments_in: segments.len() as u64,
        segments_out: out.len() as u64,
    };
    (out, report)
}
