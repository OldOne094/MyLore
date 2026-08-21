//! Recovery commands (MISSION-088). Shown by the recovery screen when the
//! database failed its startup integrity check: restore a `.mylore` archive
//! over the corrupt data, or move it aside and start fresh. Both paths close
//! the pool to unlock the files, so the app must be restarted afterwards.
//! These commands never touch the (broken) database through the managed pool.

use std::path::PathBuf;
use std::sync::Arc;
use tauri::command;
use tauri::State;
use tracing::info;

use crate::application::app_health::AppHealth;
use crate::application::backup_service::BackupService;
use crate::error::AppError;
use serde::Serialize;

/// What the UI asks on mount to decide between the normal shell and the
/// recovery screen.
#[derive(Debug, Clone, Serialize)]
pub struct HealthStatus {
    pub database_ok: bool,
}

/// Result of a recovery action; the files were swapped, so a restart is due.
#[derive(Debug, Clone, Serialize)]
pub struct RecoveryOutcome {
    /// Where the replaced data was parked (`{data_dir}/quarantine-…`).
    pub quarantined_to: String,
    pub restart_required: bool,
}

/// Startup health of the local database.
#[command]
pub async fn app_health(health: State<'_, Arc<AppHealth>>) -> Result<HealthStatus, AppError> {
    Ok(HealthStatus {
        database_ok: health.database_ok(),
    })
}

/// Move the corrupt database aside so the next startup creates a fresh one.
#[command]
pub async fn recover_start_fresh(
    backups: State<'_, Arc<BackupService>>,
) -> Result<RecoveryOutcome, AppError> {
    info!("recover_start_fresh invoked");
    let quarantined_to = backups.inner().start_fresh().await?;
    Ok(RecoveryOutcome {
        quarantined_to,
        restart_required: true,
    })
}

/// Validate and restore a `.mylore` archive over the corrupt database —
/// the same rollback-safe swap as `backup_restore`.
#[command]
pub async fn recover_restore(
    backups: State<'_, Arc<BackupService>>,
    path: String,
) -> Result<RecoveryOutcome, AppError> {
    info!("recover_restore invoked");
    let report = backups.inner().restore(&PathBuf::from(&path)).await?;
    Ok(RecoveryOutcome {
        quarantined_to: report.quarantined_to,
        restart_required: true,
    })
}
