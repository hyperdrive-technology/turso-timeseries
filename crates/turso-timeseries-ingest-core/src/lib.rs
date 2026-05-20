//! Ingest buffering and flush policies — runs outside the Turso extension.

mod buffer;
mod error;
mod policy;

pub use buffer::IngestBuffer;
pub use error::{IngestError, Result};
pub use policy::FlushPolicy;

pub use turso_timeseries_core::ingest::{encode_batch, parse_line_protocol_batch};
pub use turso_timeseries_core::model::{EncodedBatch, MetricPoint, Point};
