/// Maintenance job kinds tracked in `_tts_maintenance_jobs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaintenanceJobKind {
    Compact,
    Downsample,
    Retention,
    Stats,
    RollupRefresh,
}

/// One maintenance job row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaintenanceJob {
    pub kind: MaintenanceJobKind,
    pub target_id: Option<i64>,
    pub due_at_micros: i64,
}
