//! Bulk commands (MISSION-045). Thin handlers over `application::bulk_service` —
//! the library action bar's bulk actions: set tracking status, add a personal
//! tag, and soft-delete (to trash). (Add-to-collection commands live in
//! `commands/collection.rs` since MISSION-076.)

use sqlx::SqlitePool;
use tauri::command;
use tauri::State;
use tracing::info;

use crate::application::bulk_service::BulkService;
use crate::error::AppError;

/// Set the tracking status for many media at once (status engine applies).
#[command]
pub async fn tracking_bulk_set_status(
    state: State<'_, SqlitePool>,
    ids: Vec<String>,
    core_status: String,
) -> Result<(), AppError> {
    info!(
        count = ids.len(),
        core_status, "tracking_bulk_set_status invoked"
    );
    let service = BulkService::new(state.inner().clone());
    service.set_status(&ids, &core_status).await
}

/// Add a personal tag to many media at once (reused or created as needed).
#[command]
pub async fn media_bulk_add_tag(
    state: State<'_, SqlitePool>,
    ids: Vec<String>,
    tag: String,
) -> Result<(), AppError> {
    info!(count = ids.len(), tag, "media_bulk_add_tag invoked");
    let service = BulkService::new(state.inner().clone());
    service.add_tag(&ids, &tag).await
}

/// Soft-delete many media. Resolves with a trash id per media (for group undo).
#[command]
pub async fn media_bulk_delete(
    state: State<'_, SqlitePool>,
    ids: Vec<String>,
) -> Result<Vec<String>, AppError> {
    info!(count = ids.len(), "media_bulk_delete invoked");
    let service = BulkService::new(state.inner().clone());
    service.delete(&ids).await
}
