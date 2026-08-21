//! Backup commands (MISSION-084). Thin handlers — archive assembly and
//! validation live in `application::backup_service`. Creating a backup runs
//! as a background task (ARCHITECTURE §8): `backup_create` spawns a
//! `TaskKind::Backup` task and resolves with the initial snapshot; progress
//! rides the `task-changed` events. A cancelled or failed attempt leaves no
//! `.partial` archive behind (drop guard in the service).

use std::path::PathBuf;
use std::sync::Arc;
use tauri::command;
use tauri::State;
use tracing::info;

use crate::application::backup_service::{BackupMeta, BackupService};
use crate::application::task_service::TaskManager;
use crate::domain::task::{TaskError, TaskKind, TaskSnapshot};
use crate::error::AppError;

/// Create a validated `.mylore` backup under `{data_dir}/backups`
/// (MISSION-084). Resolves with the queued snapshot; the `BackupReport`
/// (path, size, counts) is the task's typed result on success.
#[command]
pub async fn backup_create(
    tasks: State<'_, Arc<TaskManager>>,
    backups: State<'_, Arc<BackupService>>,
) -> Result<TaskSnapshot, AppError> {
    info!("backup_create invoked");
    let service = backups.inner().clone();

    let id = tasks.spawn(
        TaskKind::Backup,
        "Create library backup".to_string(),
        move |reporter| async move {
            reporter.progress(5, Some("Snapshotting database…".to_string()));
            tokio::select! {
                result = service.create() => {
                    let report = result.map_err(|error| TaskError::failed(error.to_string()))?;
                    reporter.progress(100, Some("Backup finished".to_string()));
                    serde_json::to_value(&report)
                        .map_err(|error| TaskError::failed(error.to_string()))
                }
                _ = reporter.cancelled() => Err(TaskError::Cancelled),
            }
        },
    );

    tasks
        .get(&id)
        .ok_or_else(|| AppError::internal("backup task vanished"))
}

/// Validate a `.mylore` archive without restoring it: manifest, SQLite
/// integrity of the embedded snapshot, and manifest/snapshot count agreement.
#[command]
pub async fn backup_validate(
    backups: State<'_, Arc<BackupService>>,
    path: String,
) -> Result<BackupMeta, AppError> {
    info!("backup_validate invoked");
    backups.inner().validate(&PathBuf::from(&path)).await
}

/// Restore a `.mylore` archive as a background task (MISSION-085). The
/// current database + images are quarantined and swapped back in on failure;
/// the live pool is closed to unlock the files, so **the app must restart
/// after success** (`restart_required` in the report). The task is not
/// cancelable mid-restore by design — a dropped future would skip rollback.
#[command]
pub async fn backup_restore(
    tasks: State<'_, Arc<TaskManager>>,
    backups: State<'_, Arc<BackupService>>,
    path: String,
) -> Result<TaskSnapshot, AppError> {
    info!("backup_restore invoked");
    let service = backups.inner().clone();
    let source = PathBuf::from(&path);

    let id = tasks.spawn(
        TaskKind::Restore,
        "Restore library backup".to_string(),
        move |reporter| async move {
            reporter.progress(5, Some("Validating backup…".to_string()));
            let report = service.restore(&source).await;
            match report {
                Ok(report) => {
                    reporter.progress(100, Some("Restore finished".to_string()));
                    serde_json::to_value(&report)
                        .map_err(|error| TaskError::failed(error.to_string()))
                }
                Err(error) => Err(TaskError::failed(error.to_string())),
            }
        },
    );

    tasks
        .get(&id)
        .ok_or_else(|| AppError::internal("restore task vanished"))
}
