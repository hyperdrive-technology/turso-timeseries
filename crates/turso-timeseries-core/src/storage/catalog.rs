use crate::maintenance::{InvalidationReason, TimeRange};
use crate::model::{Hypertable, HypertableId};

/// Catalog persistence trait implemented by native/browser adapters.
pub trait CatalogStore {
    fn upsert_hypertable(&mut self, hypertable: &Hypertable) -> crate::Result<HypertableId>;
    fn mark_invalidated(
        &mut self,
        hypertable: HypertableId,
        range: TimeRange,
        reason: InvalidationReason,
    ) -> crate::Result<()>;
}
