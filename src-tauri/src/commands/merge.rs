//! Merge commands (MISSION-089). Thin handlers — planning and application
//! live in `application::merge_service`; undo lives in the trash layer.

use sqlx::SqlitePool;
use tauri::command;
use tauri::State;
use tracing::info;

use crate::application::merge_service::{MergePreview, MergeResult, MergeService};
use crate::error::AppError;

/// Preview what merging `duplicate_id` into `survivor_id` would change:
/// field conflicts, moved nodes / review / tracking / collections.
#[command]
pub async fn merge_plan(
    state: State<'_, SqlitePool>,
    survivor_id: String,
    duplicate_id: String,
) -> Result<MergePreview, AppError> {
    info!("merge_plan invoked");
    let service = MergeService::new(state.inner().clone());
    service.plan(&survivor_id, &duplicate_id).await
}

/// Apply a merge: snapshot the duplicate into trash (undoable), fold its
/// data into the survivor and delete it.
#[command]
pub async fn merge_apply(
    state: State<'_, SqlitePool>,
    survivor_id: String,
    duplicate_id: String,
) -> Result<MergeResult, AppError> {
    info!("merge_apply invoked");
    let service = MergeService::new(state.inner().clone());
    service.apply(&survivor_id, &duplicate_id).await
}
