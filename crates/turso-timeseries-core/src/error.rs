use core::fmt;

/// Crate-local result type.
pub type Result<T> = core::result::Result<T, Error>;

/// Errors from validation, parsing, and encoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    InvalidInterval { name: &'static str, value: i64 },
    EmptyMeasurement,
    EmptyTagKey,
    EmptyFieldKey,
    InvalidDuration(String),
    MalformedLineProtocol { line: usize, detail: &'static str },
    InvalidSegmentMagic,
    InvalidSegmentVersion { found: u8 },
    ChecksumMismatch,
    EmptyAggregateList,
    NonFiniteValue,
    UnknownHypertable(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInterval { name, value } => {
                write!(f, "{name} must be positive, got {value}")
            }
            Self::EmptyMeasurement => f.write_str("measurement name cannot be empty"),
            Self::EmptyTagKey => f.write_str("tag key cannot be empty"),
            Self::EmptyFieldKey => f.write_str("field key cannot be empty"),
            Self::InvalidDuration(s) => write!(f, "invalid duration: {s}"),
            Self::MalformedLineProtocol { line, detail } => {
                write!(f, "line {line}: {detail}")
            }
            Self::InvalidSegmentMagic => f.write_str("invalid segment magic"),
            Self::InvalidSegmentVersion { found } => {
                write!(f, "unsupported segment version: {found}")
            }
            Self::ChecksumMismatch => f.write_str("segment checksum mismatch"),
            Self::EmptyAggregateList => f.write_str("aggregate list cannot be empty"),
            Self::NonFiniteValue => f.write_str("floating point value must be finite"),
            Self::UnknownHypertable(name) => write!(f, "unknown hypertable: {name}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}
