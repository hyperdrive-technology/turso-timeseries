#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use super::ids::HypertableId;

/// Canonical tag set used to derive `series_key_hash`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeriesKeySpec {
    pub hypertable_id: HypertableId,
    pub tags: Vec<(String, String)>,
}

impl SeriesKeySpec {
    pub fn new(
        hypertable_id: HypertableId,
        tags: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> crate::Result<Self> {
        let mut pairs = Vec::new();
        for (k, v) in tags {
            let key = k.into();
            if key.is_empty() {
                return Err(crate::Error::EmptyTagKey);
            }
            pairs.push((key, v.into()));
        }
        pairs.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        Ok(Self {
            hypertable_id,
            tags: pairs,
        })
    }

    /// Stable FNV-1a hash over hypertable id + canonical tag JSON.
    #[must_use]
    pub fn series_key_hash(&self) -> u64 {
        let json = canonical_tags_json(&self.tags);
        let mut hash = 0xcbf29ce484222325u64;
        for byte in self.hypertable_id.0.to_le_bytes().into_iter().chain(json.bytes()) {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }
}

fn canonical_tags_json(tags: &[(String, String)]) -> String {
    let mut json = String::from("{");
    for (i, (key, value)) in tags.iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        json.push_str(&format!("\"{key}\":\"{value}\""));
    }
    json.push('}');
    json
}
