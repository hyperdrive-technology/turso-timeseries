use crate::error::{Error, Result};
use crate::model::{parse_duration_micros, TimestampMicros};

/// Floor `ts_micros` to a fixed-width bucket.
pub fn time_bucket(width: &str, ts_micros: i64) -> Result<i64> {
    let width_micros = parse_duration_micros(width)?;
    time_bucket_micros(width_micros, ts_micros)
}

/// Floor timestamp using an already-resolved width in microseconds.
pub fn time_bucket_micros(width_micros: i64, ts_micros: i64) -> Result<i64> {
    if width_micros <= 0 {
        return Err(Error::InvalidInterval {
            name: "width_micros",
            value: width_micros,
        });
    }
    Ok(ts_micros - ts_micros.rem_euclid(width_micros))
}

/// Floor to bucket using typed timestamp.
pub fn time_bucket_ts(width: &str, ts: TimestampMicros) -> Result<TimestampMicros> {
    Ok(TimestampMicros(time_bucket(width, ts.0)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aligns_five_minute_bucket() {
        let bucket = time_bucket("5m", 1_778_000_123_456_789).unwrap();
        assert_eq!(bucket, 1_778_000_100_000_000);
    }
}
