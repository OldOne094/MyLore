//! External search commands (MISSION-059). Thin handlers — the combined search
//! flow lives in `application::search_service`.

use sqlx::SqlitePool;
use tauri::command;
use tauri::State;
use tracing::info;

use std::str::FromStr;
use std::sync::Arc;

use crate::application::providers::coordinator::ProviderCoordinator;
use crate::application::search_service::{ExternalSearchView, SearchService};
use crate::domain::enums::ContentType;
use crate::error::AppError;

/// External (provider) search grouped by provider, with identity flags.
///
/// `content_type` narrows the fan-out when provided; `null` searches every
/// enabled provider (domain-agnostic). Resolves with local hits + provider
/// groups + per-provider failures.
#[command]
pub async fn search_external(
    state: State<'_, SqlitePool>,
    coordinator: State<'_, Arc<ProviderCoordinator>>,
    query: String,
    content_type: Option<String>,
) -> Result<ExternalSearchView, AppError> {
    info!(query, ?content_type, "search_external invoked");
    let content_type = content_type
        .as_deref()
        .map(ContentType::from_str)
        .transpose()?;
    let service = SearchService::new(state.inner().clone(), coordinator.inner().clone());
    service.search_external(&query, content_type).await
}
