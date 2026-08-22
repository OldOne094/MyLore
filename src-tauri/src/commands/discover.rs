//! External search commands (MISSION-059). Thin handlers — the combined search
//! flow lives in `application::search_service`.

use sqlx::SqlitePool;
use tauri::command;
use tauri::State;
use tracing::info;

use std::str::FromStr;
use std::sync::Arc;

use crate::application::providers::settings::ProviderSettingsService;
use crate::application::search_service::{ExternalSearchView, SearchService};
use crate::domain::enums::ContentType;
use crate::error::AppError;

/// Fetch full details for one title from a specific provider (MISSION-127).
/// Returns all normalized metadata (synopsis, cover, authors, genres,
/// status, counts, external links) for display on the detail screen.
#[command]
pub async fn provider_get_details(
    settings: State<'_, Arc<ProviderSettingsService>>,
    provider: String,
    id: String,
) -> Result<serde_json::Value, AppError> {
    info!(provider, id, "provider_get_details invoked");
    let coordinator = settings.coordinator();
    let token = coordinator.token();
    let media = coordinator
        .get_details(&provider, &id, &token)
        .await
        .map_err(AppError::from)?;
    Ok(serde_json::json!({
        "provider": media.provider,
        "provider_id": media.provider_id,
        "title_main": media.title_main,
        "title_original": media.title_original,
        "alt_titles": media.alt_titles,
        "content_type": media.content_type.as_str(),
        "format": media.format,
        "pub_status": media.pub_status.as_str(),
        "synopsis": media.synopsis,
        "start_date": media.start_date,
        "end_date": media.end_date,
        "release_year": media.release_year.map(|v| v as i64),
        "language": media.language,
        "country": media.country,
        "content_rating": media.content_rating,
        "pages": media.pages.map(|v| v as i64),
        "duration_min": media.duration_min.map(|v| v as i64),
        "ep_count": media.ep_count.map(|v| v as i64),
        "ch_count": media.ch_count.map(|v| v as i64),
        "cover_url": media.cover_url,
        "banner_url": media.banner_url,
        "url": media.url,
        "people": media.people.iter().map(|p| {
            serde_json::json!({ "role": p.role.as_str(), "name": p.name })
        }).collect::<Vec<_>>(),
        "genres": media.genres,
        "tags": media.tags,
    }))
}

/// External (provider) search grouped by provider, with identity flags.
///
/// `content_type` narrows the fan-out when provided; `null` searches every
/// enabled provider (domain-agnostic). Resolves with local hits + provider
/// groups + per-provider failures.
#[command]
pub async fn search_external(
    state: State<'_, SqlitePool>,
    settings: State<'_, Arc<ProviderSettingsService>>,
    query: String,
    content_type: Option<String>,
) -> Result<ExternalSearchView, AppError> {
    info!(query, ?content_type, "search_external invoked");
    let content_type = content_type
        .as_deref()
        .map(ContentType::from_str)
        .transpose()?;
    let service = SearchService::new(state.inner().clone(), settings.coordinator());
    service.search_external(&query, content_type).await
}
