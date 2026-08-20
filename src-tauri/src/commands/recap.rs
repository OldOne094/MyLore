//! Recap commands (MISSION-082). Thin handler — the year assembly lives in
//! `application::recap_service`.

use sqlx::SqlitePool;
use tauri::command;
use tauri::State;
use tracing::info;

use crate::application::recap_service::{RecapService, YearRecap};
use crate::error::AppError;

/// Resolve the year-in-review recap for one year: headline totals, a monthly
/// completion chart, top genres of finished media, the most-active media, and
/// the longest streak of consecutive active days.
#[command]
pub async fn recap_year(state: State<'_, SqlitePool>, year: u16) -> Result<YearRecap, AppError> {
    info!("recap_year invoked");
    let service = RecapService::new(state.inner().clone());
    service.year(year).await
}
