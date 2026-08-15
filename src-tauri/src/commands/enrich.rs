//! Enrich metadata commands (MISSION-061). Thin handlers — the refresh + diff
//! flow lives in `application::enrich_service`.

use sqlx::SqlitePool;
use tauri::State;
use tauri::command;
use tracing::info;

use std::sync::Arc;

use crate::application::enrich_service::{EnrichService, EnrichView};
use crate::application::providers::coordinator::ProviderCoordinator;
use crate::error::AppError;

/// Refresh a media's provider-owned metadata from its provider and report what
/// changed (before → after per field). Never touches user data (tracking,
/// review, collections, personal tags, asset ids).
#[command]
pub async fn media_enrich(
    state: State<'_, SqlitePool>,
    coordinator: State<'_, Arc<ProviderCoordinator>>,
    media_id: String,
) -> Result<EnrichView, AppError> {
    info!(media_id, "media_enrich invoked");
    let service = EnrichService::new(state.inner().clone(), coordinator.inner().clone());
    service.enrich_from_provider(&media_id).await
}
