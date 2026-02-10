pub mod api_helpers;
pub mod gap;
pub mod rate_limit;
pub mod syncer;

use chrono::NaiveDate;
use serde::Serialize;

/// Summary of changes detected during incremental sync.
///
/// Passed to [`SyncProgress::on_incremental_sync`] so consumers can see
/// exactly which resource types changed, not just task counts.
#[derive(Debug, Clone)]
pub struct IncrementalSyncSummary {
    pub tasks_changed: usize,
    pub project_changed: bool,
    pub sections_changed: bool,
    pub status_updates_changed: bool,
}

/// Trait for receiving progress callbacks during sync operations.
///
/// Methods are ordered to match the typical temporal call sequence during a
/// sync operation. All methods have default no-op implementations so consumers
/// only need to override the callbacks they care about. Library users who don't
/// need progress can pass `&NoopProgress`.
///
/// Note: only tasks count toward `items_synced` in [`SyncReport`]. Status
/// updates, sections, and project metadata are auxiliary data tracked via
/// their own callbacks.
pub trait SyncProgress: Send + Sync {
    fn on_entity_start(&self, _entity_key: &str, _index: usize, _total: usize) {}
    fn on_tasks_fetched(&self, _entity_key: &str, _count: usize) {}
    fn on_comments_skipped(&self, _entity_key: &str, _skipped: usize, _total: usize) {}
    fn on_comments_progress(&self, _entity_key: &str, _current: usize, _total: usize) {}
    fn on_status_updates_synced(&self, _entity_key: &str, _count: usize) {}
    fn on_incremental_sync(&self, _entity_key: &str, _summary: &IncrementalSyncSummary) {}
    fn on_entity_complete(&self, _report: &SyncReport) {}
}

/// No-op implementation of `SyncProgress` for callers that don't need progress.
pub struct NoopProgress;
impl SyncProgress for NoopProgress {}

/// Options controlling a sync operation.
#[derive(Debug, Clone)]
pub struct SyncOptions {
    pub since: Option<NaiveDate>,
    pub days: Option<u32>,
    /// Force a full sync even if an incremental sync token is available.
    pub full: bool,
}

impl SyncOptions {
    pub fn since_date(&self) -> Option<NaiveDate> {
        if let Some(d) = self.since {
            Some(d)
        } else {
            self.days
                .map(|days| chrono::Local::now().date_naive() - chrono::Duration::days(days as i64))
        }
    }
}

/// Report returned after a sync operation completes.
#[derive(Debug, Clone, Serialize)]
pub struct SyncReport {
    pub entity_key: String,
    pub status: SyncStatus,
    pub items_synced: u64,
    pub items_failed: u64,
    pub batches_completed: u32,
    pub batches_total: u32,
    pub error: Option<String>,
}

impl SyncReport {
    /// Create a SyncReport with the appropriate status derived from counts.
    pub fn from_counts(
        entity_key: String,
        items_synced: u64,
        items_failed: u64,
        batches_completed: u32,
        batches_total: u32,
    ) -> Self {
        let status = if items_failed == 0 {
            SyncStatus::Success
        } else if items_synced > 0 || batches_completed > 0 {
            SyncStatus::PartialFailure
        } else {
            SyncStatus::Failed
        };
        let error = if items_failed > 0 {
            Some(format!("{items_failed} items failed"))
        } else {
            None
        };
        Self {
            entity_key,
            status,
            items_synced,
            items_failed,
            batches_completed,
            batches_total,
            error,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum SyncStatus {
    Success,
    PartialFailure,
    Failed,
}
