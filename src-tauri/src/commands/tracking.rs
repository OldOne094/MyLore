//! Tracking commands (MISSION-048). Thin handlers over
//! `application::tracking_service` — single-media status read + transition
//! (the auto-complete rule runs on progress writes inside the service layer).

use sqlx::SqlitePool;
use tauri::command;
use tauri::State;
use tracing::info;

use crate::application::tracking_service::{TrackingService, TrackingView};
use crate::error::AppError;

/// Read the tracking row for one media. Resolves with the row or null when the
/// media is untracked; rejects with an AppError string.
#[command]
pub async fn tracking_get(
    state: State<'_, SqlitePool>,
    media_id: String,
) -> Result<Option<TrackingView>, AppError> {
    info!(media_id, "tracking_get invoked");
    let service = TrackingService::new(state.inner().clone());
    service.get(&media_id).await
}

/// Apply a status transition for one media (status engine applies, incl. the
/// Repeat guard and started/finished stamps). Resolves with the updated row or
/// rejects with an AppError string.
#[command]
pub async fn tracking_set_status(
    state: State<'_, SqlitePool>,
    media_id: String,
    core_status: String,
) -> Result<TrackingView, AppError> {
    info!(media_id, core_status, "tracking_set_status invoked");
    let service = TrackingService::new(state.inner().clone());
    service.set_status(&media_id, &core_status).await
}

/// Toggle Normal (autoTrack) vs Manual tracking mode for one media
/// (MISSION-052). Resolves with the updated row (turning Normal back on
/// re-syncs the status to the current progress) or rejects with an AppError
/// string.
#[command]
pub async fn tracking_set_auto_track(
    state: State<'_, SqlitePool>,
    media_id: String,
    auto_track: bool,
) -> Result<TrackingView, AppError> {
    info!(media_id, auto_track, "tracking_set_auto_track invoked");
    let service = TrackingService::new(state.inner().clone());
    service.set_auto_track(&media_id, auto_track).await
}
