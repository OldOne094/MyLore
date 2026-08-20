//! Calendar commands (MISSION-081). Thin handler — the month assembly lives in
//! `application::calendar_service`.

use sqlx::SqlitePool;
use tauri::command;
use tauri::State;
use tracing::info;

use crate::application::calendar_service::{CalendarMonth, CalendarService};
use crate::error::AppError;

/// Resolve one calendar month: content-node air/release dates plus the user
/// activity trail, bucketed per local day. Resolves with a `CalendarMonth` or
/// rejects with an AppError string.
#[command]
pub async fn calendar_month(
    state: State<'_, SqlitePool>,
    year: u16,
    month: u8,
) -> Result<CalendarMonth, AppError> {
    info!("calendar_month invoked");
    let service = CalendarService::new(state.inner().clone());
    service.month(year, month).await
}
