//! Background task commands (MISSION-070). Thin handlers over the
//! `application::task_service::TaskManager`; progress and terminal state also
//! stream to the UI as `task_changed` events.

use std::sync::Arc;

use tauri::command;
use tauri::State;
use tracing::info;

use crate::application::task_service::TaskManager;
use crate::domain::task::TaskSnapshot;
use crate::error::AppError;

/// Every task snapshot, newest first.
#[command]
pub fn task_list(tasks: State<'_, Arc<TaskManager>>) -> Result<Vec<TaskSnapshot>, AppError> {
    info!("task_list invoked");
    Ok(tasks.list())
}

/// The current snapshot of one task.
#[command]
pub fn task_get(tasks: State<'_, Arc<TaskManager>>, id: String) -> Result<TaskSnapshot, AppError> {
    info!(id, "task_get invoked");
    tasks
        .get(&id)
        .ok_or_else(|| AppError::validation(format!("unknown task: {id}")))
}

/// Request cancellation of a task. The runner observes the flag and aborts at
/// the next checkpoint (dropping its in-flight transaction).
#[command]
pub fn task_cancel(
    tasks: State<'_, Arc<TaskManager>>,
    id: String,
) -> Result<TaskSnapshot, AppError> {
    info!(id, "task_cancel invoked");
    tasks
        .cancel(&id)
        .ok_or_else(|| AppError::validation(format!("unknown task: {id}")))
}
