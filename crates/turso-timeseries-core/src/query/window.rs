#[cfg(not(feature = "std"))]
use alloc::collections::VecDeque;
#[cfg(feature = "std")]
use std::collections::VecDeque;

use crate::model::TimestampMicros;

/// Fixed-time moving average window.
pub struct MovingAvg {
    window_micros: i64,
    points: VecDeque<(TimestampMicros, f64)>,
    sum: f64,
}

impl MovingAvg {
    #[must_use]
    pub fn new(window_micros: i64) -> Self {
        Self {
            window_micros,
            points: VecDeque::new(),
            sum: 0.0,
        }
    }

    /// Push a point and return the current moving average if the window is non-empty.
    pub fn push(&mut self, ts: TimestampMicros, value: f64) -> Option<f64> {
        self.points.push_back((ts, value));
        self.sum += value;

        while let Some((old_ts, old_value)) = self.points.front().copied() {
            if ts.0 - old_ts.0 > self.window_micros {
                self.points.pop_front();
                self.sum -= old_value;
            } else {
                break;
            }
        }

        if self.points.is_empty() {
            None
        } else {
            Some(self.sum / self.points.len() as f64)
        }
    }
}
