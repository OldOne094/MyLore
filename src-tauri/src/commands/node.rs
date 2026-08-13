//! Content-node commands (MISSION-046). Thin handlers — tree assembly lives in
//! `application::node_service`; per-node progress commands land with
//! MISSION-047.

use sqlx::SqlitePool;
use tauri::command;
use tauri::State;
use tracing::info;

use crate::application::node_service::{ContentNode, NodeService};
use crate::error::AppError;

/// Read the full content tree for one media (seasons→episodes, volumes→
/// chapters). Resolves with the nested tree, roots ordered by position, or
/// rejects with an AppError string.
#[command]
pub async fn media_nodes(
    state: State<'_, SqlitePool>,
    id: String,
) -> Result<Vec<ContentNode>, AppError> {
    info!(id, "media_nodes invoked");
    let service = NodeService::new(state.inner().clone());
    service.tree_for_media(&id).await
}
