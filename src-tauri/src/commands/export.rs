//! Export commands (MISSION-071). Thin handler — the streaming write lives in
//! `application::export_service`. The export runs as a background task
//! (ARCHITECTURE §8): `export_media` opens the user-chosen path (from the
//! native save dialog on the frontend), spawns a `TaskKind::ExportFile` task
//! that streams the library row by row, and resolves with the initial snapshot.
//! Progress rides the `task-changed` events; cancellation drops the partial
//! file (the `PartialExport` guard removes it).

use sqlx::SqlitePool;
use tauri::command;
use tauri::State;
use tracing::info;

use std::path::Path;
use std::sync::Arc;

use crate::application::export_service::ExportService;
use crate::application::task_service::TaskManager;
use crate::domain::export::ExportFormat;
use crate::domain::task::{TaskError, TaskKind, TaskSnapshot};
use crate::error::AppError;

/// Export the whole library to `path` as a background task (MISSION-071).
/// `format` is `json` | `csv` | `markdown` (the save dialog's chosen
/// extension). Resolves with the queued snapshot; the `ExportReport` (format,
/// total, path) is the task's typed result on success.
#[command]
pub async fn export_media(
    tasks: State<'_, Arc<TaskManager>>,
    state: State<'_, SqlitePool>,
    format: String,
    path: String,
) -> Result<TaskSnapshot, AppError> {
    info!(format, path, "export_media invoked");
    let format = format.parse::<ExportFormat>()?;
    let pool = state.inner().clone();
    let title = format!("Export {} library", format.as_str());

    let id = tasks.spawn(TaskKind::ExportFile, title, move |reporter| async move {
        let service = ExportService::new(pool);
        reporter.progress(0, Some("Preparing export…".to_string()));

        let stream = service.stream_to_path(Path::new(&path), format, |done, total| {
            let percent = if total == 0 {
                100
            } else {
                done.checked_mul(100)
                    .and_then(|p| p.checked_div(total))
                    .unwrap_or(100) as u32
            };
            reporter.progress(percent, Some(format!("Exporting {done}/{total} titles")));
        });
        tokio::pin!(stream);

        tokio::select! {
            result = &mut stream => {
                let report = result.map_err(|error| TaskError::failed(error.to_string()))?;
                reporter.progress(100, Some("Export finished".to_string()));
                serde_json::to_value(&report)
                    .map_err(|error| TaskError::failed(error.to_string()))
            }
            _ = reporter.cancelled() => Err(TaskError::Cancelled),
        }
    });

    tasks
        .get(&id)
        .ok_or_else(|| AppError::internal("export task vanished"))
}
