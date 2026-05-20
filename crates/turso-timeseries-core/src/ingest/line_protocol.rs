#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use crate::error::{Error, Result};
use crate::model::{FieldValue, MetricPoint, TimestampMicros};

/// Parse one Influx line protocol line.
pub fn parse_line_protocol(line: &str) -> Result<MetricPoint> {
    parse_line_protocol_numbered(line, 1)
}

pub fn parse_line_protocol_batch(lines: &str) -> Result<Vec<MetricPoint>> {
    let mut out = Vec::new();
    for (idx, line) in lines.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        out.push(parse_line_protocol_numbered(trimmed, idx + 1)?);
    }
    Ok(out)
}

fn parse_line_protocol_numbered(line: &str, line_no: usize) -> Result<MetricPoint> {
    let (head, timestamp) = split_timestamp(line).ok_or(Error::MalformedLineProtocol {
        line: line_no,
        detail: "missing timestamp",
    })?;

    let (measurement_tags, fields) = head
        .split_once(' ')
        .ok_or(Error::MalformedLineProtocol {
            line: line_no,
            detail: "missing fields",
        })?;

    let (measurement, tags) = parse_measurement_tags(measurement_tags, line_no)?;
    let fields = parse_fields(fields, line_no)?;
    if measurement.is_empty() {
        return Err(Error::EmptyMeasurement);
    }

    Ok(MetricPoint {
        table: measurement,
        time: TimestampMicros(timestamp),
        tags,
        fields,
    })
}

fn split_timestamp(line: &str) -> Option<(&str, i64)> {
    let mut parts = line.rsplitn(2, ' ');
    let ts = parts.next()?;
    let head = parts.next()?;
    let ts_micros = match ts.len() {
        19 => ts.parse::<i64>().ok().map(|n| n / 1_000)?,
        16 => ts.parse::<i64>().ok().map(|n| n / 1_000_000)?,
        13 => ts.parse::<i64>().ok().map(|n| n / 1_000)?,
        10 => ts.parse::<i64>().ok().map(|n| n * 1_000_000)?,
        _ => ts.parse::<i64>().ok()?,
    };
    Some((head, ts_micros))
}

fn parse_measurement_tags(part: &str, line_no: usize) -> Result<(String, Vec<(String, String)>)> {
    let mut segments = part.split(',');
    let measurement = segments
        .next()
        .ok_or(Error::MalformedLineProtocol {
            line: line_no,
            detail: "missing measurement",
        })?
        .to_string();

    let mut tags = Vec::new();
    for tag in segments {
        let (key, value) = tag.split_once('=').ok_or(Error::MalformedLineProtocol {
            line: line_no,
            detail: "invalid tag",
        })?;
        if key.is_empty() {
            return Err(Error::EmptyTagKey);
        }
        tags.push((unescape(key), unescape(value)));
    }
    Ok((measurement, tags))
}

fn parse_fields(part: &str, line_no: usize) -> Result<Vec<(String, FieldValue)>> {
    let mut fields = Vec::new();
    for field in split_fields(part) {
        let (key, raw) = field.split_once('=').ok_or(Error::MalformedLineProtocol {
            line: line_no,
            detail: "invalid field",
        })?;
        if key.is_empty() {
            return Err(Error::EmptyFieldKey);
        }
        fields.push((unescape(key), parse_field_value(raw)?));
    }
    Ok(fields)
}

fn parse_field_value(raw: &str) -> Result<FieldValue> {
    if raw.ends_with('i') {
        let digits = &raw[..raw.len() - 1];
        return Ok(FieldValue::I64(digits.parse().map_err(|_| {
            Error::MalformedLineProtocol {
                line: 0,
                detail: "invalid integer field",
            }
        })?));
    }
    if raw == "true" {
        return Ok(FieldValue::Bool(true));
    }
    if raw == "false" {
        return Ok(FieldValue::Bool(false));
    }
    if raw.starts_with('"') && raw.ends_with('"') {
        return Ok(FieldValue::Text(unescape(&raw[1..raw.len() - 1])));
    }
    if let Ok(v) = raw.parse::<f64>() {
        if !v.is_finite() {
            return Err(Error::NonFiniteValue);
        }
        return Ok(FieldValue::F64(v));
    }
    Err(Error::MalformedLineProtocol {
        line: 0,
        detail: "unsupported field value",
    })
}

fn split_fields(part: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut in_string = false;
    let mut escaped = false;
    for (idx, ch) in part.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            ',' if !in_string => {
                out.push(&part[start..idx]);
                start = idx + 1;
            }
            _ => {}
        }
    }
    out.push(&part[start..]);
    out
}

fn unescape(input: &str) -> String {
    let mut out = String::new();
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            }
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_line() {
        let point = parse_line_protocol(
            "metrics,device_id=a value=1 1778000000000000000",
        )
        .unwrap();
        assert_eq!(point.table, "metrics");
        assert_eq!(point.tags, vec![("device_id".to_string(), "a".to_string())]);
        assert_eq!(point.time.0, 1_778_000_000_000_000);
    }
}
