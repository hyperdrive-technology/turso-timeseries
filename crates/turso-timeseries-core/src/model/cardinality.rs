/// Per-hypertable cardinality guardrails.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CardinalityPolicy {
    pub max_series: Option<u64>,
    pub max_tag_keys: Option<u16>,
    pub max_tag_value_len: Option<usize>,
    pub reject_new_series_after_limit: bool,
    pub warn_at_fraction: f64,
}

impl Default for CardinalityPolicy {
    fn default() -> Self {
        Self {
            max_series: None,
            max_tag_keys: Some(32),
            max_tag_value_len: Some(256),
            reject_new_series_after_limit: false,
            warn_at_fraction: 0.9,
        }
    }
}

impl CardinalityPolicy {
    /// Returns true when a new series should be rejected under the current count.
    #[must_use]
    pub fn should_reject_new_series(&self, current_series: u64) -> bool {
        match (self.max_series, self.reject_new_series_after_limit) {
            (Some(max), true) => current_series >= max,
            _ => false,
        }
    }

    #[must_use]
    pub fn should_warn(&self, current_series: u64) -> bool {
        match self.max_series {
            Some(max) if max > 0 => {
                (current_series as f64 / max as f64) >= self.warn_at_fraction
            }
            _ => false,
        }
    }
}
