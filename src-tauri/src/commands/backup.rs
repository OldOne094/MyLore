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
