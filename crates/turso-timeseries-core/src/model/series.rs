#[cfg(not(feature = "std"))]
use alloc::string::String;

/// Deterministic series identity from measurement + sorted tags.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SeriesKey {
    measurement: String,
    tags_json: String,
}

impl SeriesKey {
    pub fn new(
        measurement: impl Into<String>,
        tags: &[(&str, &str)],
    ) -> crate::Result<Self> {
        let measurement = measurement.into();
        if measurement.trim().is_empty() {
            return Err(crate::Error::EmptyMeasurement);
        }
        let mut pairs: Vec<(String, String)> = tags
            .iter()
            .map(|(k, v)| {
                if k.is_empty() {
                    return Err(crate::Error::EmptyTagKey);
                }
                Ok(((*k).to_string(), (*v).to_string()))
            })
            .collect::<crate::Result<_>>()?;
        pairs.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        Ok(Self {
            measurement,
            tags_json: canonical_tags_json(&pairs),
        })
    }

    #[must_use]
    pub fn measurement(&self) -> &str {
        &self.measurement
    }

    #[must_use]
    pub fn tags_json(&self) -> &str {
        &self.tags_json
    }
}

fn canonical_tags_json(tags: &[(String, String)]) -> String {
    let mut json = String::from("{");
    for (i, (key, value)) in tags.iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        json.push('"');
        push_json_escaped(&mut json, key);
        json.push_str("\":\"");
        push_json_escaped(&mut json, value);
        json.push('"');
    }
    json.push('}');
    json
}

fn push_json_escaped(out: &mut String, value: &str) {
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < ' ' => {
                out.push_str("\\u");
                out.push_str(&format!("{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
}

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
