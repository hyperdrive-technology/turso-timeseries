#[cfg(not(feature = "std"))]
use alloc::string::String;

/// Logical column type in catalog metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalType {
    Integer,
    Real,
    Text,
    Bool,
    Blob,
    Timestamp,
}

/// Role of a column in a hypertable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnRole {
    Time,
    Tag,
    Field,
}

/// Field column definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldColumnDef {
    pub name: String,
    pub logical_type: LogicalType,
    pub nullable: bool,
}
