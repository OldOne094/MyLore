//! Media commands (MISSION-038). Thin handlers — validation and persistence
//! live in `application::media_service` (spec §83).

use sqlx::SqlitePool;
use tauri::command;
use tauri::State;
use tracing::info;

use crate::application::media_service::{
    AddMediaInput, MediaListItem, MediaListInput, MediaService,
};
use crate::error::AppError;

/// Create a media entry from manual input. Resolves with the new media id.
///
/// Field set mirrors the IPC contract (`scripts/ipc-contract.json`).
#[allow(clippy::too_many_arguments)]
#[command]
pub async fn media_create(
    state: State<'_, SqlitePool>,
    title: String,
    content_type: String,
    format: Option<String>,
    pub_status: Option<String>,
    synopsis: Option<String>,
    release_year: Option<i64>,
    language: Option<String>,
    country: Option<String>,
    pages: Option<i64>,
    duration_min: Option<i64>,
    ep_count: Option<i64>,
    ch_count: Option<i64>,
    genres: Vec<String>,
) -> Result<String, AppError> {
    info!(content_type, "media_create invoked");
    let service = MediaService::new(state.inner().clone());
    let input = AddMediaInput {
        title,
        content_type,
        format,
        pub_status,
        synopsis,
        release_year,
        language,
        country,
        pages,
        duration_min,
        ep_count,
        ch_count,
        genres,
    };
    Ok(service.add_media(input).await?.as_str().to_string())
}

/// List library entries, optionally filtered. Resolves with summary rows.
///
/// Filters mirror the IPC contract (`scripts/ipc-contract.json`).
#[allow(clippy::too_many_arguments)]
#[command]
pub async fn media_list(
    state: State<'_, SqlitePool>,
    content_type: Option<String>,
    format: Option<String>,
    pub_status: Option<String>,
    genre: Option<String>,
    tag: Option<String>,
    year: Option<i64>,
    favorite: Option<bool>,
    search: Option<String>,
    sort: Option<String>,
    ascending: Option<bool>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<MediaListItem>, AppError> {
    info!(?content_type, ?sort, "media_list invoked");
    let service = MediaService::new(state.inner().clone());
    let input = MediaListInput {
        content_type,
        format,
        pub_status,
        genre,
        tag,
        year,
        favorite,
        search,
        sort,
        ascending,
        limit,
        offset,
    };
    service.list_media(input).await
}

/// Distinct filter values present in the library (MISSION-041).
#[command]
pub async fn media_facets(
    state: State<'_, SqlitePool>,
) -> Result<crate::infrastructure::repositories::media::MediaFacets, AppError> {
    info!("media_facets invoked");
    let service = MediaService::new(state.inner().clone());
    service.list_facets().await
}