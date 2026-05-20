use crate::error::{Error, Result};
use crate::model::MetricPoint;

/// Validate points before write.
pub fn validate_points(points: &[MetricPoint]) -> Result<()> {
    for point in points {
        if point.table.trim().is_empty() {
            return Err(Error::EmptyMeasurement);
        }
        if point.fields.is_empty() {
            return Err(Error::MalformedLineProtocol {
                line: 0,
                detail: "point has no fields",
            });
        }
        for (key, value) in &point.fields {
            if key.is_empty() {
                return Err(Error::EmptyFieldKey);
            }
            if let crate::model::FieldValue::F64(v) = value {
                if !v.is_finite() {
                    return Err(Error::NonFiniteValue);
                }
            }
        }
    }
    Ok(())
}
