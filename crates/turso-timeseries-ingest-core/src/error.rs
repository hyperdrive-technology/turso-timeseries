pub type Result<T> = core::result::Result<T, IngestError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngestError {
    Core(turso_timeseries_core::Error),
    BufferEmpty,
    FrameTooLarge,
}

impl From<turso_timeseries_core::Error> for IngestError {
    fn from(value: turso_timeseries_core::Error) -> Self {
        Self::Core(value)
    }
}

impl core::fmt::Display for IngestError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Core(e) => write!(f, "{e}"),
            Self::BufferEmpty => f.write_str("ingest buffer is empty"),
            Self::FrameTooLarge => f.write_str("ingest frame exceeds policy limit"),
        }
    }
}

