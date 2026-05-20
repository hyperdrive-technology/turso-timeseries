/// Stable series identifier used for joins and storage keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct SeriesId(pub i64);

/// Field column identifier within a hypertable schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct FieldId(pub u32);

/// Hypertable identifier in catalog metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct HypertableId(pub i64);
