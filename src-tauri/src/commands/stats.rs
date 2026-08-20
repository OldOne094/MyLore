//! Stats commands (MISSION-080). Thin handler — the whole-library computation
//! lives in `application::stats_service`.

use sqlx::SqlitePool;
use tauri::command;
use tauri::State;
use tracing::info;

use crate::application::stats_service::{StatsService, StatsView};
use crate::error::AppError;

/// Resolve the library statistics overview — counts per status / content type,
/// hours and pages consumed, completion, average rating, favorites, and the
/// rating + release-year distributions. Resolves with a `StatsView` or rejects
/// with an AppError string.
#[command]
pub async fn stats_summary(state: State<'_, SqlitePool>) -> Result<StatsView, AppError> {
    info!("stats_summary invoked");
    let service = StatsService::new(state.inner().clone());
    service.summary().await
}
