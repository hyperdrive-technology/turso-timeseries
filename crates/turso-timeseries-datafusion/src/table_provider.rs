//! Placeholder `TableProvider` until DataFusion is wired behind a feature flag.

#[derive(Debug, Default)]
pub struct TimeseriesTableProvider {
    pub hypertable: String,
}
