//! Background task manager (MISSION-070, ARCHITECTURE §8).
//!
//! Long-running work (import, and later export / metadata sync / backup) is
//! spawned off the command path as a task with states
//! `queued → running(progress) → success | failed | cancelled`. Every state
//! change (and each progress tick) is pushed through an emitter — in the app
//! that's the `task_changed` Tauri event; tests inject a collecting sink.
//!
//! Cancellation is cooperative and drop-based: `TaskManager::cancel` flips a
//! flag the runner observes (`TaskReporter::cancelled`), so a `tokio::select!`
//! can drop the in-flight future — dropping a sqlx transaction rolls the
//! batch back — or the runner can bail with `TaskError::Cancelled`.

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex, RwLock};
use std::time::SystemTime;

use chrono::{DateTime, Utc};
use serde_json::Value;
use tokio::sync::watch;
use tracing::info;
use uuid::Uuid;

use crate::domain::task::{TaskError, TaskKind, TaskSnapshot, TaskState};

/// A single tracked task and its live state.
struct TaskEntry {
    id: String,
    kind: TaskKind,
    title: String,
    created_at: String,
    cancel: watch::Sender<bool>,
    state: RwLock<TaskState>,
    progress: RwLock<Option<u32>>,
    message: RwLock<Option<String>>,
    error: RwLock<Option<String>>,
    result: RwLock<Option<Value>>,
    updated_at: RwLock<String>,
}

impl TaskEntry {
    fn new(id: String, kind: TaskKind, title: String, cancel: watch::Sender<bool>) -> Self {
        let now = now_rfc3339();
        Self {
            id,
            kind,
            title,
            created_at: now.clone(),
            cancel,
            state: RwLock::new(TaskState::Queued),
            progress: RwLock::new(None),
            message: RwLock::new(None),
            error: RwLock::new(None),
            result: RwLock::new(None),
            updated_at: RwLock::new(now),
        }
    }

    fn snapshot(&self) -> TaskSnapshot {
        TaskSnapshot {
            id: self.id.clone(),
            kind: self.kind.as_str().to_string(),
            title: self.title.clone(),
            state: *self.state.read().unwrap(),
            progress: *self.progress.read().unwrap(),
            message: self.message.read().unwrap().clone(),
            error: self.error.read().unwrap().clone(),
            result: self.result.read().unwrap().clone(),
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.read().unwrap().clone(),
        }
    }
}

fn now_rfc3339() -> String {
    DateTime::<Utc>::from(SystemTime::now()).to_rfc3339()
}

/// Handle a task runner uses to report progress, observe cancellation, and
/// mutate the task's live state. Cheap to clone; the entry is shared.
#[derive(Clone)]
pub struct TaskReporter {
    entry: Arc<TaskEntry>,
    cancel_rx: watch::Receiver<bool>,
    emit: Arc<dyn Fn(TaskSnapshot) + Send + Sync>,
}

impl TaskReporter {
    fn new(
        entry: Arc<TaskEntry>,
        cancel_rx: watch::Receiver<bool>,
        emit: Arc<dyn Fn(TaskSnapshot) + Send + Sync>,
    ) -> Self {
        Self {
            entry,
            cancel_rx,
            emit,
        }
    }

    /// Mark the task as running (the first event a spawned task emits).
    pub fn start(&self) {
        *self.entry.state.write().unwrap() = TaskState::Running;
        self.emit();
    }

    /// Report progress (0..=100) plus an optional status line.
    pub fn progress(&self, percent: u32, message: Option<String>) {
        *self.entry.progress.write().unwrap() = Some(percent.min(100));
        *self.entry.message.write().unwrap() = message;
        self.emit();
    }

    /// Replace just the status line, keeping progress as-is.
    pub fn message(&self, message: String) {
        *self.entry.message.write().unwrap() = Some(message);
        self.emit();
    }

    /// Set the terminal state and result/error, then emit.
    fn finish(&self, state: TaskState, result: Option<Value>, error: Option<String>) {
        // Result and error land BEFORE the terminal state: the state write is
        // the commit marker, so no observer can ever see a terminal snapshot
        // whose payload has not landed yet.
        *self.entry.result.write().unwrap() = result;
        *self.entry.error.write().unwrap() = error;
        *self.entry.state.write().unwrap() = state;
        self.emit();
    }

    /// True once `TaskManager::cancel` has been called for this task.
    pub fn is_cancelled(&self) -> bool {
        *self.cancel_rx.borrow()
    }

    /// Resolves once the task has been cancelled. Pair with `tokio::select!` to
    /// drop the in-flight future (drop-based cancellation).
    pub async fn cancelled(&self) {
        let mut rx = self.cancel_rx.clone();
        if *rx.borrow() {
            return;
        }
        let _ = rx.changed().await;
    }

    fn emit(&self) {
        *self.entry.updated_at.write().unwrap() = now_rfc3339();
        (self.emit)(self.entry.snapshot());
    }
}

/// Registry + lifecycle for background tasks. Managed as `Arc<TaskManager>`
/// Tauri state so commands can spawn, list, read, and cancel.
pub struct TaskManager {
    tasks: Mutex<HashMap<String, Arc<TaskEntry>>>,
    emit: Arc<dyn Fn(TaskSnapshot) + Send + Sync>,
}

impl TaskManager {
    /// Build a manager that pushes every change through `emit`. The app wires
    /// this to the `task_changed` Tauri event; tests collect the snapshots.
    pub fn with_emitter<F>(emit: F) -> Self
    where
        F: Fn(TaskSnapshot) + Send + Sync + 'static,
    {
        Self {
            tasks: Mutex::new(HashMap::new()),
            emit: Arc::new(emit),
        }
    }

    /// Register a task and run `run` off the command path. Returns the task id;
    /// the caller can `get` the initial (queued) snapshot immediately.
    pub fn spawn<F, Fut>(&self, kind: TaskKind, title: String, run: F) -> String
    where
        F: FnOnce(TaskReporter) -> Fut + Send + 'static,
        Fut: Future<Output = Result<Value, TaskError>> + Send + 'static,
    {
        let id = format!("t-{}", Uuid::new_v4());
        let (cancel_tx, cancel_rx) = watch::channel(false);
        let entry = Arc::new(TaskEntry::new(id.clone(), kind, title, cancel_tx));
        self.tasks.lock().unwrap().insert(id.clone(), entry.clone());

        let reporter = TaskReporter::new(entry.clone(), cancel_rx, self.emit.clone());
        tauri::async_runtime::spawn(async move {
            reporter.start();
            match run(reporter.clone()).await {
                Ok(result) => reporter.finish(TaskState::Success, Some(result), None),
                Err(TaskError::Cancelled) => reporter.finish(TaskState::Cancelled, None, None),
                Err(TaskError::Failed(message)) => {
                    info!(task_id = %entry.id, "task failed: {message}");
                    reporter.finish(TaskState::Failed, None, Some(message));
                }
            }
        });

        id
    }

    /// Snapshot of one task, or `None` when the id is unknown.
    pub fn get(&self, id: &str) -> Option<TaskSnapshot> {
        self.tasks
            .lock()
            .unwrap()
            .get(id)
            .map(|entry| entry.snapshot())
    }

    /// Every task snapshot, newest first.
    pub fn list(&self) -> Vec<TaskSnapshot> {
        let mut all: Vec<TaskSnapshot> = self
            .tasks
            .lock()
            .unwrap()
            .values()
            .map(|entry| entry.snapshot())
            .collect();
        all.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        all
    }

    /// Request cancellation. The runner observes the flag (and drops its future
    /// when it is inside `select!`); a queued task cancels once it starts.
    pub fn cancel(&self, id: &str) -> Option<TaskSnapshot> {
        let entry = self.tasks.lock().unwrap().get(id).cloned()?;
        let _ = entry.cancel.send(true);
        Some(entry.snapshot())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::task::TaskKind;
    use serde_json::json;

    fn collecting_manager() -> (TaskManager, Arc<Mutex<Vec<TaskSnapshot>>>) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        (
            TaskManager::with_emitter(move |snapshot| sink.lock().unwrap().push(snapshot)),
            seen,
        )
    }

    async fn wait_terminal(manager: &TaskManager, id: &str) -> TaskSnapshot {
        for _ in 0..10_000 {
            if let Some(snapshot) = manager.get(id) {
                if snapshot.state.is_terminal() {
                    return snapshot;
                }
            }
            tokio::task::yield_now().await;
        }
        panic!("task {id} never reached a terminal state");
    }

    #[tokio::test]
    async fn task_reports_progress_and_a_typed_result() {
        let (manager, seen) = collecting_manager();
        let id = manager.spawn(
            TaskKind::ImportFile,
            "import test".to_string(),
            |reporter| async move {
                reporter.progress(30, Some("Parsing…".to_string()));
                reporter.progress(70, Some("Writing…".to_string()));
                Ok(json!({ "committed": 3 }))
            },
        );

        let snapshot = wait_terminal(&manager, &id).await;
        assert_eq!(snapshot.state, TaskState::Success);
        assert_eq!(snapshot.result, Some(json!({ "committed": 3 })));

        let states: Vec<TaskState> = seen.lock().unwrap().iter().map(|s| s.state).collect();
        assert_eq!(
            states,
            vec![
                TaskState::Running,
                TaskState::Running,
                TaskState::Running,
                TaskState::Success
            ]
        );
        let progresses: Vec<Option<u32>> =
            seen.lock().unwrap().iter().map(|s| s.progress).collect();
        assert_eq!(progresses, vec![None, Some(30), Some(70), Some(70)]);
    }

    #[tokio::test]
    async fn cancel_flips_the_flag_and_runners_can_bail() {
        let (manager, seen) = collecting_manager();
        let id = manager.spawn(
            TaskKind::ImportFile,
            "cancel me".to_string(),
            |reporter| async move {
                // select! observes the cancel flag deterministically — a plain
                // spin loop could finish before `cancel()` lands and flake the
                // assertion (the runner would resolve Success instead).
                tokio::select! {
                    _ = reporter.cancelled() => Err(TaskError::Cancelled),
                    () = std::future::pending() => Ok(json!({ "finished": true })),
                }
            },
        );

        manager.cancel(&id).expect("task exists");
        let snapshot = wait_terminal(&manager, &id).await;
        assert_eq!(snapshot.state, TaskState::Cancelled);

        let last = seen.lock().unwrap().last().unwrap().state;
        assert_eq!(last, TaskState::Cancelled);
    }

    #[tokio::test]
    async fn failed_runner_records_the_message() {
        let (manager, _seen) = collecting_manager();
        let id = manager.spawn(
            TaskKind::ImportFile,
            "boom".to_string(),
            |_reporter| async move { Err(TaskError::failed("database error: locked")) },
        );

        let snapshot = wait_terminal(&manager, &id).await;
        assert_eq!(snapshot.state, TaskState::Failed);
        assert_eq!(snapshot.error.as_deref(), Some("database error: locked"));
    }

    #[tokio::test]
    async fn list_and_get_reflect_all_states() {
        let (manager, _seen) = collecting_manager();
        let id = manager.spawn(
            TaskKind::ImportFile,
            "list me".to_string(),
            |_reporter| async move { Ok(json!(null)) },
        );
        wait_terminal(&manager, &id).await;

        let list = manager.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, id);
        assert_eq!(manager.get("t-missing"), None);
    }
}
