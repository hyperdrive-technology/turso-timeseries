#[cfg(feature = "line-protocol")]
mod line_protocol;

mod batch;
mod encoded_batch;
mod validator;
mod write_buffer;

pub use encoded_batch::{encode_batch, BATCH_MAGIC};

pub use batch::IngestBatch;
pub use validator::validate_points;
pub use write_buffer::WriteBuffer;

#[cfg(feature = "line-protocol")]
pub use line_protocol::{parse_line_protocol, parse_line_protocol_batch};
