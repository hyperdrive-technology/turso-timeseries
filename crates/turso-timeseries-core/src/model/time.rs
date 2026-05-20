use crate::error::{Error, Result};

/// Timestamp in microseconds since Unix epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct TimestampMicros(pub i64);

/// Parse a duration string such as `5m`, `1h`, `30s` into microseconds.
pub fn parse_duration_micros(width: &str) -> Result<i64> {
    let width = width.trim();
    if width.is_empty() {
        return Err(Error::InvalidDuration(width.to_string()));
    }

    let split = width
        .chars()
        .position(|c| !c.is_ascii_digit())
        .unwrap_or(width.len());
    if split == 0 {
        return Err(Error::InvalidDuration(width.to_string()));
    }

    let amount: i64 = width[..split]
        .parse()
        .map_err(|_| Error::InvalidDuration(width.to_string()))?;
    if amount <= 0 {
        return Err(Error::InvalidDuration(width.to_string()));
    }

    let unit = &width[split..];
    let micros_per_unit: i64 = match unit {
        "us" | "µs" => 1,
        "ms" => 1_000,
        "s" => 1_000_000,
        "m" => 60 * 1_000_000,
        "h" => 60 * 60 * 1_000_000,
        "d" => 24 * 60 * 60 * 1_000_000,
        "w" => 7 * 24 * 60 * 60 * 1_000_000,
        _ => return Err(Error::InvalidDuration(width.to_string())),
    };

    amount
        .checked_mul(micros_per_unit)
        .ok_or_else(|| Error::InvalidDuration(width.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_durations() {
        assert_eq!(parse_duration_micros("5m").unwrap(), 300_000_000);
        assert_eq!(parse_duration_micros("1h").unwrap(), 3_600_000_000);
        assert_eq!(parse_duration_micros("30s").unwrap(), 30_000_000);
    }
}
