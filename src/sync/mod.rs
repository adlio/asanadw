pub mod api_helpers;
pub mod gap;
pub mod rate_limit;
pub mod syncer;

use chrono::NaiveDate;
use serde::Serialize;

/// Options controlling a sync operation.
#[derive(Debug, Clone)]
pub struct SyncOptions {
    pub since: Option<NaiveDate>,
    pub days: Option<u32>,
}

impl SyncOptions {
    pub fn since_date(&self) -> Option<NaiveDate> {
        if let Some(d) = self.since {
            Some(d)
        } else { self.days.map(|days| chrono::Local::now().date_naive() - chrono::Duration::days(days as i64)) }
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
