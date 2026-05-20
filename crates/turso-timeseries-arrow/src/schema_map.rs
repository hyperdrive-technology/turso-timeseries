//! Maps hypertable columns to Arrow schema fields.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrowField {
    pub name: String,
    pub nullable: bool,
}
