//! Review commands (MISSION-074). Thin handlers over
//! `application::review_service` — read/save/clear a media's user-owned review
//! and acknowledge its content-warning set (MISSION-079).

use sqlx::SqlitePool;
use tauri::command;
use tauri::State;
use tracing::info;

use crate::application::review_service::{ReviewService, ReviewView, SaveReviewInput};
use crate::error::AppError;

/// Read a media's review. Resolves with the row or null when the user hasn't
/// reviewed it; rejects with an AppError string.
#[command]
pub async fn review_get(
    state: State<'_, SqlitePool>,
    media_id: String,
) -> Result<Option<ReviewView>, AppError> {
    info!(media_id, "review_get invoked");
    let service = ReviewService::new(state.inner().clone());
    service.get(&media_id).await
}

/// Save (create or update) a media's review. Resolves with the stored row (an
/// entirely empty review clears the row and resolves with an empty view) or
/// rejects with an AppError string.
#[allow(clippy::too_many_arguments)]
#[command]
pub async fn review_save(
    state: State<'_, SqlitePool>,
    media_id: String,
    rating: Option<i64>,
    review: Option<String>,
    short_review: Option<String>,
    notes: Option<String>,
    favorite: bool,
    is_spoiler: bool,
    moods: Vec<String>,
    pace: Option<String>,
    content_warnings: Vec<String>,
) -> Result<ReviewView, AppError> {
    info!(media_id, "review_save invoked");
    let service = ReviewService::new(state.inner().clone());
    let input = SaveReviewInput {
        media_id,
        rating,
        review,
        short_review,
        notes,
        favorite,
        is_spoiler,
        moods,
        pace,
        content_warnings,
    };
    service.save(input).await
}

/// Acknowledge a media's current content-warning set (MISSION-079) — stamps
/// `warnings_acknowledged_at` now and resolves with the updated row, or
/// rejects when there is no review / no warnings to acknowledge.
#[command]
pub async fn review_acknowledge_warnings(
    state: State<'_, SqlitePool>,
    media_id: String,
) -> Result<ReviewView, AppError> {
    info!(media_id, "review_acknowledge_warnings invoked");
    let service = ReviewService::new(state.inner().clone());
    service.acknowledge_warnings(&media_id).await
}

/// Delete a media's review row. Resolves or rejects with an AppError string.
#[command]
pub async fn review_delete(state: State<'_, SqlitePool>, media_id: String) -> Result<(), AppError> {
    info!(media_id, "review_delete invoked");
    let service = ReviewService::new(state.inner().clone());
    service.delete(&media_id).await
}
