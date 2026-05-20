use crate::model::{FieldValue, TimestampMicros};

/// Tracks the latest value for a series.
#[derive(Debug, Default)]
pub struct LastValueState {
    time: Option<TimestampMicros>,
    value: Option<FieldValue>,
}

impl LastValueState {
    pub fn push(&mut self, time: TimestampMicros, value: FieldValue) {
        match self.time {
            None => {
                self.time = Some(time);
                self.value = Some(value);
            }
            Some(existing) if time >= existing => {
                self.time = Some(time);
                self.value = Some(value);
            }
            _ => {}
        }
    }

    #[must_use]
    pub fn finish(&self) -> Option<&FieldValue> {
        self.value.as_ref()
    }
}
