//! Dashboard commands (MISSION-050). Thin handler — aggregation lives in
//! `application::dashboard_service`.

use sqlx::SqlitePool;
use tauri::command;
use tauri::State;
use tracing::info;

use crate::application::dashboard_service::{DashboardService, DashboardSummary};
use crate::error::AppError;

/// Resolve the dashboard widget lists — continue reading/watching, recently
/// completed, recently added — in one round-trip. `limit` is optional and
/// clamped per widget (1..=20). Resolves with the `DashboardSummary` or
/// rejects with an AppError string.
#[command]
pub async fn dashboard_summary(
    state: State<'_, SqlitePool>,
    limit: Option<u32>,
) -> Result<DashboardSummary, AppError> {
    info!(?limit, "dashboard_summary invoked");
    let service = DashboardService::new(state.inner().clone());
    service.summary(limit).await
}
