//! Background task model (MISSION-070, ARCHITECTURE §8).
//!
//! A task is a cancelable unit of long-running work with a progress value and a
//! typed result. The `TaskManager` runs them off the command path; every change
//! is emitted as a `task_changed` event so the UI can stream progress, cancel,
//! and read the result without blocking the IPC handler.

use serde_json::Value;

use serde::Serialize;

/// What kind of work a task performs — drives the shape of its result payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    /// JSON/CSV file import (result: `ImportReport`).
    ImportFile,
    /// JSON/CSV/Markdown library export (result: `ExportReport`).
    ExportFile,
}

impl TaskKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ImportFile => "import_file",
            Self::ExportFile => "export_file",
        }
    }
}

/// Lifecycle of a background task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    /// Accepted but not yet running.
    Queued,
    /// The runner is executing; `progress` may be reported.
    Running,
    /// Finished; `result` holds the typed payload.
    Success,
    /// Finished with an error; `error` holds a user-safe message.
    Failed,
    /// Cancelled before completion.
    Cancelled,
}

impl TaskState {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Success | Self::Failed | Self::Cancelled)
    }
}

/// A point-in-time view of a task, shipped over `task_changed` and `task_get`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TaskSnapshot {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub state: TaskState,
    /// 0..=100 while running; null while indeterminate.
    pub progress: Option<u32>,
    /// A short human-readable status line for the current stage.
    pub message: Option<String>,
    /// Set on `Failed`; a user-safe error message.
    pub error: Option<String>,
    /// The typed result payload (e.g. an `ImportReport`) on `Success`.
    pub result: Option<Value>,
    pub created_at: String,
    pub updated_at: String,
}

/// The only errors a task runner may return: a cooperative cancellation signal
/// or a user-safe failure message.
#[derive(Debug, thiserror::Error)]
pub enum TaskError {
    #[error("task was cancelled")]
    Cancelled,

    #[error("{0}")]
    Failed(String),
}

impl TaskError {
    pub fn failed(message: impl Into<String>) -> Self {
        Self::Failed(message.into())
    }
}
