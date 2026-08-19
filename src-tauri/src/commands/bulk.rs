//! Bulk commands (MISSION-045). Thin handlers over `application::bulk_service` —
//! the library action bar's bulk actions: set tracking status, add a personal
//! tag, and soft-delete (to trash). MISSION-078 adds an optional facet
//! `filter`: when present the media set is resolved server-side (apply to the
//! whole filtered selection) and every command resolves with a per-item change
//! summary. (Add-to-collection commands live in `commands/collection.rs`.)

use sqlx::SqlitePool;
use tauri::command;
use tauri::State;
use tracing::info;

use crate::application::bulk_service::{BulkDeleteResult, BulkFilter, BulkResult, BulkService};
use crate::error::AppError;

/// Set the tracking status for many media at once (status engine applies).
/// Resolves with a per-item summary; media that can't reach the target status
/// are reported in `failures` instead of aborting the batch.
#[command]
pub async fn tracking_bulk_set_status(
    state: State<'_, SqlitePool>,
    ids: Vec<String>,
    core_status: String,
    filter: Option<BulkFilter>,
) -> Result<BulkResult, AppError> {
    info!(
        count = ids.len(),
        filtered = filter.is_some(),
        core_status,
        "tracking_bulk_set_status invoked"
    );
    let service = BulkService::new(state.inner().clone());
    service.set_status(filter, &ids, &core_status).await
}

/// Add a personal tag to many media at once (reused or created as needed).
/// Resolves with a per-item summary.
#[command]
pub async fn media_bulk_add_tag(
    state: State<'_, SqlitePool>,
    ids: Vec<String>,
    tag: String,
    filter: Option<BulkFilter>,
) -> Result<BulkResult, AppError> {
    info!(
        count = ids.len(),
        filtered = filter.is_some(),
        tag,
        "media_bulk_add_tag invoked"
    );
    let service = BulkService::new(state.inner().clone());
    service.add_tag(filter, &ids, &tag).await
}

/// Soft-delete many media. Resolves with a summary plus a trash id per
/// successfully deleted media (for a group undo over exactly what was removed).
#[command]
pub async fn media_bulk_delete(
    state: State<'_, SqlitePool>,
    ids: Vec<String>,
    filter: Option<BulkFilter>,
) -> Result<BulkDeleteResult, AppError> {
    info!(
        count = ids.len(),
        filtered = filter.is_some(),
        "media_bulk_delete invoked"
    );
    let service = BulkService::new(state.inner().clone());
    service.delete(filter, &ids).await
}
