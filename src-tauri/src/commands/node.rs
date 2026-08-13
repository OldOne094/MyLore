//! Content-node commands (MISSION-046, MISSION-047). Thin handlers — tree
//! assembly lives in `application::node_service`, progress writes in
//! `application::progress_service`.

use sqlx::SqlitePool;
use tauri::command;
use tauri::State;
use tracing::info;

use crate::application::node_service::{ContentNode, NodeService};
use crate::application::progress_service::{NodeProgressNextView, ProgressService};
use crate::error::AppError;

/// Read the full content tree for one media (seasons→episodes, volumes→
/// chapters), with per-node progress state attached. Resolves with the nested
/// tree, roots ordered by position, or rejects with an AppError string.
#[command]
pub async fn media_nodes(
    state: State<'_, SqlitePool>,
    id: String,
) -> Result<Vec<ContentNode>, AppError> {
    info!(id, "media_nodes invoked");
    let service = NodeService::new(state.inner().clone());
    service.tree_for_media(&id).await
}

/// Set the progress state of one node (`read`/`watched`/`skipped`/`unread`).
/// Resolves or rejects with an AppError string.
#[command]
pub async fn node_progress_set(
    state: State<'_, SqlitePool>,
    node_id: String,
    node_state: String,
) -> Result<(), AppError> {
    info!(node_id, node_state, "node_progress_set invoked");
    let service = ProgressService::new(state.inner().clone());
    service.set_node_progress(&node_id, &node_state).await
}

/// Set the progress state of every node between two nodes in the media's
/// display order. Resolves with the affected node ids (for optimistic UI) or
/// rejects with an AppError string.
#[command]
pub async fn node_progress_range(
    state: State<'_, SqlitePool>,
    media_id: String,
    from_id: String,
    to_id: String,
    node_state: String,
) -> Result<Vec<String>, AppError> {
    info!(
        media_id,
        from_id, to_id, node_state, "node_progress_range invoked"
    );
    let service = ProgressService::new(state.inner().clone());
    service
        .set_range_progress(&media_id, &from_id, &to_id, &node_state)
        .await
}

/// Mark the next not-yet-consumed countable node of a media done (`watched`
/// for episodes, `read` otherwise) and run the auto-status rule. Resolves with
/// the refreshed progress summary, null when nothing is left to mark, or
/// rejects with an AppError string.
#[command]
pub async fn node_progress_next(
    state: State<'_, SqlitePool>,
    media_id: String,
) -> Result<Option<NodeProgressNextView>, AppError> {
    info!(media_id, "node_progress_next invoked");
    let service = ProgressService::new(state.inner().clone());
    service.mark_next_unit(&media_id).await
}
