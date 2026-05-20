use super::predicate::ScanConstraints;

/// Hypertable-level statistics for cost estimation.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct HypertableStats {
    pub row_count: u64,
    pub min_time_micros: Option<i64>,
    pub max_time_micros: Option<i64>,
    pub series_count: u64,
    pub chunk_count: u64,
}

impl HypertableStats {
    #[must_use]
    pub fn total_time_width(&self) -> i64 {
        match (self.min_time_micros, self.max_time_micros) {
            (Some(min), Some(max)) if max > min => max - min,
            _ => 1,
        }
    }
}

/// Estimated scan cost for join planning.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScanEstimate {
    pub estimated_rows: u64,
    pub estimated_cost: f64,
}

/// Estimate rows/cost from stats and pushed constraints.
#[must_use]
pub fn estimate_scan_cost(stats: &HypertableStats, constraints: &ScanConstraints) -> ScanEstimate {
    let time_fraction = constraints
        .time_range_micros()
        .map(|(start, end)| {
            let width = (end - start).max(1) as f64;
            (width / stats.total_time_width() as f64).clamp(0.0, 1.0)
        })
        .unwrap_or(1.0);

    let series_fraction = constraints
        .series_eq_count
        .map(|n| n as f64 / stats.series_count.max(1) as f64)
        .unwrap_or(1.0)
        .clamp(0.0, 1.0);

    let estimated_rows =
        ((stats.row_count as f64) * time_fraction * series_fraction).max(1.0) as u64;

    ScanEstimate {
        estimated_rows,
        estimated_cost: estimated_rows as f64,
    }
}
