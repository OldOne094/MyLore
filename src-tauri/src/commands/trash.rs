//! Trash commands (MISSION-044). Thin handlers over `application::trash_service`
//! and `media_delete` conveniently grouped here with the trash lifecycle.

use sqlx::SqlitePool;
use tauri::command;
use tauri::State;
use tracing::info;

use crate::application::trash_service::{TrashItem, TrashService};
use crate::error::AppError;

/// Soft-delete a media: store its before-image in trash, cascade the row away.
/// Resolves with the trash id, which `trash_restore` accepts (for undo toasts).
#[command]
pub async fn media_delete(state: State<'_, SqlitePool>, id: String) -> Result<String, AppError> {
    info!(id, "media_delete invoked");
    let service = TrashService::new(state.inner().clone());
    service.delete_media(&id).await
}

/// List active (not restored) trash entries for the trash page.
#[command]
pub async fn trash_list(state: State<'_, SqlitePool>) -> Result<Vec<TrashItem>, AppError> {
    info!("trash_list invoked");
    let service = TrashService::new(state.inner().clone());
    service.list_trash().await
}

/// Restore a soft-deleted aggregate from its trash before-image.
#[command]
pub async fn trash_restore(state: State<'_, SqlitePool>, id: String) -> Result<(), AppError> {
    info!(id, "trash_restore invoked");
    let service = TrashService::new(state.inner().clone());
    service.restore_media(&id).await
}

/// Permanently forget a trash entry (the aggregate row is already gone).
#[command]
pub async fn trash_purge(state: State<'_, SqlitePool>, id: String) -> Result<(), AppError> {
    info!(id, "trash_purge invoked");
    let service = TrashService::new(state.inner().clone());
    service.purge(&id).await
}
