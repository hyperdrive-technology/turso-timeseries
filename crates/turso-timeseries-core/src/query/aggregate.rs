#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::model::{FieldValue, TimestampMicros};

/// Supported aggregate functions (P0 set).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregateKind {
    Count,
    Sum,
    Avg,
    Min,
    Max,
    First,
    Last,
}

/// Finished aggregate output.
#[derive(Debug, Clone, PartialEq)]
pub enum AggregateValue {
    Null,
    F64(f64),
    I64(i64),
}

/// Trait for incremental aggregate state machines.
pub trait AggregateState {
    fn push(&mut self, value: &FieldValue, time: TimestampMicros);
    fn finish(&self) -> AggregateValue;
}

#[derive(Default)]
pub struct CountState {
    count: u64,
}

impl AggregateState for CountState {
    fn push(&mut self, value: &FieldValue, _time: TimestampMicros) {
        if !matches!(value, FieldValue::Null) {
            self.count += 1;
        }
    }

    fn finish(&self) -> AggregateValue {
        AggregateValue::I64(self.count as i64)
    }
}

#[derive(Default)]
pub struct SumState {
    sum: f64,
    has_value: bool,
}

impl AggregateState for SumState {
    fn push(&mut self, value: &FieldValue, _time: TimestampMicros) {
        if let Some(v) = value.as_f64() {
            self.sum += v;
            self.has_value = true;
        }
    }

    fn finish(&self) -> AggregateValue {
        if self.has_value {
            AggregateValue::F64(self.sum)
        } else {
            AggregateValue::Null
        }
    }
}

#[derive(Default)]
pub struct AvgState {
    sum: f64,
    count: u64,
}

impl AggregateState for AvgState {
    fn push(&mut self, value: &FieldValue, _time: TimestampMicros) {
        if let Some(v) = value.as_f64() {
            self.sum += v;
            self.count += 1;
        }
    }

    fn finish(&self) -> AggregateValue {
        if self.count == 0 {
            AggregateValue::Null
        } else {
            AggregateValue::F64(self.sum / self.count as f64)
        }
    }
}

#[derive(Default)]
pub struct MinState {
    min: Option<f64>,
}

impl AggregateState for MinState {
    fn push(&mut self, value: &FieldValue, _time: TimestampMicros) {
        if let Some(v) = value.as_f64() {
            self.min = Some(self.min.map_or(v, |m| m.min(v)));
        }
    }

    fn finish(&self) -> AggregateValue {
        self.min.map(AggregateValue::F64).unwrap_or(AggregateValue::Null)
    }
}

#[derive(Default)]
pub struct MaxState {
    max: Option<f64>,
}

impl AggregateState for MaxState {
    fn push(&mut self, value: &FieldValue, _time: TimestampMicros) {
        if let Some(v) = value.as_f64() {
            self.max = Some(self.max.map_or(v, |m| m.max(v)));
        }
    }

    fn finish(&self) -> AggregateValue {
        self.max.map(AggregateValue::F64).unwrap_or(AggregateValue::Null)
    }
}

pub struct FirstState {
    first: Option<(TimestampMicros, f64)>,
}

impl Default for FirstState {
    fn default() -> Self {
        Self { first: None }
    }
}

impl AggregateState for FirstState {
    fn push(&mut self, value: &FieldValue, time: TimestampMicros) {
        if let Some(v) = value.as_f64() {
            match self.first {
                None => self.first = Some((time, v)),
                Some((t, _)) if time < t => self.first = Some((time, v)),
                _ => {}
            }
        }
    }

    fn finish(&self) -> AggregateValue {
        self.first
            .map(|(_, v)| AggregateValue::F64(v))
            .unwrap_or(AggregateValue::Null)
    }
}

pub struct LastState {
    last: Option<(TimestampMicros, f64)>,
}

impl Default for LastState {
    fn default() -> Self {
        Self { last: None }
    }
}

impl AggregateState for LastState {
    fn push(&mut self, value: &FieldValue, time: TimestampMicros) {
        if let Some(v) = value.as_f64() {
            match self.last {
                None => self.last = Some((time, v)),
                Some((t, _)) if time >= t => self.last = Some((time, v)),
                _ => {}
            }
        }
    }

    fn finish(&self) -> AggregateValue {
        self.last
            .map(|(_, v)| AggregateValue::F64(v))
            .unwrap_or(AggregateValue::Null)
    }
}

/// Boxed aggregate state for dynamic dispatch.
pub enum DynamicAggregateState {
    Count(CountState),
    Sum(SumState),
    Avg(AvgState),
    Min(MinState),
    Max(MaxState),
    First(FirstState),
    Last(LastState),
}

impl DynamicAggregateState {
    #[must_use]
    pub fn new(kind: AggregateKind) -> Self {
        match kind {
            AggregateKind::Count => Self::Count(CountState::default()),
            AggregateKind::Sum => Self::Sum(SumState::default()),
            AggregateKind::Avg => Self::Avg(AvgState::default()),
            AggregateKind::Min => Self::Min(MinState::default()),
            AggregateKind::Max => Self::Max(MaxState::default()),
            AggregateKind::First => Self::First(FirstState::default()),
            AggregateKind::Last => Self::Last(LastState::default()),
        }
    }
}

pub fn aggregate_push(state: &mut DynamicAggregateState, value: &FieldValue, time: TimestampMicros) {
    match state {
        DynamicAggregateState::Count(s) => s.push(value, time),
        DynamicAggregateState::Sum(s) => s.push(value, time),
        DynamicAggregateState::Avg(s) => s.push(value, time),
        DynamicAggregateState::Min(s) => s.push(value, time),
        DynamicAggregateState::Max(s) => s.push(value, time),
        DynamicAggregateState::First(s) => s.push(value, time),
        DynamicAggregateState::Last(s) => s.push(value, time),
    }
}

pub fn aggregate_finish(state: &DynamicAggregateState) -> AggregateValue {
    match state {
        DynamicAggregateState::Count(s) => s.finish(),
        DynamicAggregateState::Sum(s) => s.finish(),
        DynamicAggregateState::Avg(s) => s.finish(),
        DynamicAggregateState::Min(s) => s.finish(),
        DynamicAggregateState::Max(s) => s.finish(),
        DynamicAggregateState::First(s) => s.finish(),
        DynamicAggregateState::Last(s) => s.finish(),
    }
}

/// Parse aggregate name strings used in rollup policies.
pub fn parse_aggregate_kind(name: &str) -> Option<AggregateKind> {
    match name {
        "count" => Some(AggregateKind::Count),
        "sum" => Some(AggregateKind::Sum),
        "avg" => Some(AggregateKind::Avg),
        "min" => Some(AggregateKind::Min),
        "max" => Some(AggregateKind::Max),
        "first" => Some(AggregateKind::First),
        "last" => Some(AggregateKind::Last),
        _ => None,
    }
}

/// Build aggregate states from policy names.
pub fn aggregate_states_from_names(names: &[&str]) -> Vec<(AggregateKind, DynamicAggregateState)> {
    names
        .iter()
        .filter_map(|name| parse_aggregate_kind(name).map(|k| (k, DynamicAggregateState::new(k))))
        .collect()
}
