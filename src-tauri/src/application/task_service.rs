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
use std::sync::{Arc, Mutex, PoisonError, RwLock};
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
            state: *self.state.read().unwrap_or_else(PoisonError::into_inner),
            progress: *self.progress.read().unwrap_or_else(PoisonError::into_inner),
            message: self
                .message
                .read()
                .unwrap_or_else(PoisonError::into_inner)
                .clone(),
            error: self
                .error
                .read()
                .unwrap_or_else(PoisonError::into_inner)
                .clone(),
            result: self
                .result
                .read()
                .unwrap_or_else(PoisonError::into_inner)
                .clone(),
            created_at: self.created_at.clone(),
            updated_at: self
                .updated_at
                .read()
                .unwrap_or_else(PoisonError::into_inner)
                .clone(),
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
        *self
            .entry
            .state
            .write()
            .unwrap_or_else(PoisonError::into_inner) = TaskState::Running;
        self.emit();
    }

    /// Report progress (0..=100) plus an optional status line.
    pub fn progress(&self, percent: u32, message: Option<String>) {
        *self
            .entry
            .progress
            .write()
            .unwrap_or_else(PoisonError::into_inner) = Some(percent.min(100));
        *self
            .entry
            .message
            .write()
            .unwrap_or_else(PoisonError::into_inner) = message;
        self.emit();
    }

    /// Replace just the status line, keeping progress as-is.
    pub fn message(&self, message: String) {
        *self
            .entry
            .message
            .write()
            .unwrap_or_else(PoisonError::into_inner) = Some(message);
        self.emit();
    }

    /// Set the terminal state and result/error, then emit.
    fn finish(&self, state: TaskState, result: Option<Value>, error: Option<String>) {
        // Result and error land BEFORE the terminal state: the state write is
        // the commit marker, so no observer can ever see a terminal snapshot
        // whose payload has not landed yet.
        *self
            .entry
            .result
            .write()
            .unwrap_or_else(PoisonError::into_inner) = result;
        *self
            .entry
            .error
            .write()
            .unwrap_or_else(PoisonError::into_inner) = error;
        *self
            .entry
            .state
            .write()
            .unwrap_or_else(PoisonError::into_inner) = state;
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
        *self
            .entry
            .updated_at
            .write()
            .unwrap_or_else(PoisonError::into_inner) = now_rfc3339();
        (self.emit)(self.entry.snapshot());
    }
}

/// Ensures a task reaches a terminal state even if its runner never returns
/// normally.
///
/// `finish` is only reachable through the `match` in [`TaskManager::spawn`], so a
/// panic unwinding through the runner used to skip it: the task stayed `Running`
/// forever, the UI showed work that had already stopped, and — because pruning is
/// terminal-only — the entry was never evicted either. Dropping the future (a
/// runtime shutdown) has the same effect. This guard closes both holes, and a
/// panic is additionally recorded by the process panic hook.
struct FinishGuard {
    reporter: TaskReporter,
    armed: bool,
}

impl FinishGuard {
    fn arm(reporter: TaskReporter) -> Self {
        Self {
            reporter,
            armed: true,
        }
    }

    /// The runner reported its own terminal state; nothing left to do.
    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for FinishGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        info!(
            task_id = %self.reporter.entry.id,
            "task runner ended without a result; marking it failed"
        );
        self.reporter.finish(
            TaskState::Failed,
            None,
            Some("the task stopped unexpectedly before it could finish".to_string()),
        );
    }
}

/// Registry + lifecycle for background tasks. Managed as `Arc<TaskManager>`
/// Tauri state so commands can spawn, list, read, and cancel.
pub struct TaskManager {
    tasks: Mutex<HashMap<String, Arc<TaskEntry>>>,
    emit: Arc<dyn Fn(TaskSnapshot) + Send + Sync>,
}

/// Retention cap for terminal (finished) tasks (MISSION-143). A long-lived
/// session otherwise grows the task map without bound — every import/export/
/// backup snapshot (including its full typed result payload) stays resident
/// forever. Running/queued tasks are never pruned; only the newest terminal
/// tasks survive beyond this count.
const MAX_TERMINAL_TASKS: usize = 50;

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
    ///
    /// Takes `&Arc<Self>` so the runner can prune terminal tasks *after* it
    /// finishes (MISSION-143); production calls through Tauri's
    /// `State<Arc<TaskManager>>`, tests wrap the manager in an `Arc`.
    pub fn spawn<F, Fut>(self: &Arc<Self>, kind: TaskKind, title: String, run: F) -> String
    where
        F: FnOnce(TaskReporter) -> Fut + Send + 'static,
        Fut: Future<Output = Result<Value, TaskError>> + Send + 'static,
    {
        let id = format!("t-{}", Uuid::new_v4());
        let (cancel_tx, cancel_rx) = watch::channel(false);
        let entry = Arc::new(TaskEntry::new(id.clone(), kind, title, cancel_tx));
        {
            let mut tasks = self.tasks.lock().unwrap_or_else(PoisonError::into_inner);
            tasks.insert(id.clone(), entry.clone());
            prune_locked(&mut tasks);
        }

        let reporter = TaskReporter::new(entry.clone(), cancel_rx, self.emit.clone());
        let manager = self.clone();
        tauri::async_runtime::spawn(async move {
            reporter.start();
            // Armed before the runner: if it unwinds or is dropped, the task
            // still ends as `Failed` instead of hanging in `Running` forever.
            let mut guard = FinishGuard::arm(reporter.clone());
            match run(reporter.clone()).await {
                Ok(result) => reporter.finish(TaskState::Success, Some(result), None),
                Err(TaskError::Cancelled) => reporter.finish(TaskState::Cancelled, None, None),
                Err(TaskError::Failed(message)) => {
                    info!(task_id = %entry.id, "task failed: {message}");
                    reporter.finish(TaskState::Failed, None, Some(message));
                }
            }
            guard.disarm();
            // The task is terminal now; keep the map bounded even if no new
            // task spawns for a while.
            prune(&manager);
        });

        id
    }

    /// Snapshot of one task, or `None` when the id is unknown.
    pub fn get(&self, id: &str) -> Option<TaskSnapshot> {
        self.tasks
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(id)
            .map(|entry| entry.snapshot())
    }

    /// Every task snapshot, newest first.
    pub fn list(&self) -> Vec<TaskSnapshot> {
        let mut all: Vec<TaskSnapshot> = self
            .tasks
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .values()
            .map(|entry| entry.snapshot())
            .collect();
        all.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        all
    }

    /// Request cancellation. The runner observes the flag (and drops its future
    /// when it is inside `select!`); a queued task cancels once it starts.
    pub fn cancel(&self, id: &str) -> Option<TaskSnapshot> {
        let entry = self
            .tasks
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(id)
            .cloned()?;
        let _ = entry.cancel.send(true);
        Some(entry.snapshot())
    }
}

/// Evict the oldest terminal tasks beyond the retention cap (MISSION-143).
/// Terminal tasks are ranked by their `updated_at` stamp; running/queued
/// entries are never touched.
fn prune(manager: &TaskManager) {
    let mut tasks = manager.tasks.lock().unwrap_or_else(PoisonError::into_inner);
    prune_locked(&mut tasks);
}

/// Lock-held half of [`prune`].
fn prune_locked(tasks: &mut HashMap<String, Arc<TaskEntry>>) {
    // Collect terminal entries; if they fit under the cap nothing is pruned
    // even when live tasks push the total count past it.
    let mut terminal: Vec<(String, String)> = tasks
        .iter()
        .filter(|(_, entry)| {
            entry
                .state
                .read()
                .unwrap_or_else(PoisonError::into_inner)
                .is_terminal()
        })
        .map(|(id, entry)| {
            (
                id.clone(),
                entry
                    .updated_at
                    .read()
                    .unwrap_or_else(PoisonError::into_inner)
                    .clone(),
            )
        })
        .collect();
    if terminal.len() <= MAX_TERMINAL_TASKS {
        return;
    }
    terminal.sort_by(|a, b| a.1.cmp(&b.1)); // oldest last-updated first
    let excess = terminal.len() - MAX_TERMINAL_TASKS;
    for (id, _) in terminal.into_iter().take(excess) {
        tasks.remove(&id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::task::TaskKind;
    use serde_json::json;

    fn collecting_manager() -> (Arc<TaskManager>, Arc<Mutex<Vec<TaskSnapshot>>>) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        (
            Arc::new(TaskManager::with_emitter(move |snapshot| {
                sink.lock().unwrap().push(snapshot)
            })),
            seen,
        )
    }

    async fn wait_terminal(manager: &TaskManager, id: &str) -> TaskSnapshot {
        // A real 1ms sleep (not yield_now): on the current-thread runtime,
        // yield_now re-queues this test into the scheduler's LIFO slot and can
        // starve the spawned runner indefinitely on slower CI machines.
        for _ in 0..10_000 {
            if let Some(snapshot) = manager.get(id) {
                if snapshot.state.is_terminal() {
                    return snapshot;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
        panic!("task {id} never reached a terminal state");
    }

    /// MISSION-154: a runner that panics must not strand the task in `Running`.
    /// Before the guard, `finish` was unreachable after an unwind — the task
    /// stayed live forever, was never pruned (pruning is terminal-only), and the
    /// UI showed work that had already stopped.
    #[tokio::test]
    async fn a_panicking_runner_still_reaches_a_terminal_state() {
        let (manager, _seen) = collecting_manager();
        let id = manager.spawn(
            TaskKind::ImportFile,
            "exploding task".to_string(),
            |_reporter| async move {
                panic!("a bug inside the runner");
            },
        );

        let snapshot = wait_terminal(&manager, &id).await;
        assert_eq!(snapshot.state, TaskState::Failed);
        assert!(
            snapshot
                .error
                .unwrap_or_default()
                .contains("stopped unexpectedly"),
            "the failure must say the task stopped, not invent a result"
        );

        // …and the manager is still usable afterwards.
        let next = manager.spawn(
            TaskKind::ExportFile,
            "after the panic".to_string(),
            |_reporter| async move { Ok(json!({ "ok": true })) },
        );
        assert_eq!(
            wait_terminal(&manager, &next).await.state,
            TaskState::Success
        );
        assert_eq!(manager.list().len(), 2);
    }

    /// MISSION-154: a poisoned lock used to take the whole task API down — every
    /// later `get`/`list`/`cancel`/`spawn` panicked on `.unwrap()`. Recovering is
    /// the project-wide convention (MISSION-141).
    #[tokio::test]
    async fn the_task_api_survives_a_poisoned_lock() {
        let (manager, _seen) = collecting_manager();

        let poisoned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _held = manager.tasks.lock().unwrap();
            panic!("poison the manager lock");
        }));
        assert!(poisoned.is_err());
        assert!(manager.tasks.is_poisoned(), "the lock really is poisoned");

        // Every entry point still answers instead of panicking.
        assert!(manager.list().is_empty());
        assert!(manager.get("t-missing").is_none());
        assert!(manager.cancel("t-missing").is_none());

        let id = manager.spawn(
            TaskKind::Backup,
            "spawned after the poison".to_string(),
            |_reporter| async move { Ok(json!({ "ok": true })) },
        );
        assert_eq!(wait_terminal(&manager, &id).await.state, TaskState::Success);
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

    #[tokio::test]
    async fn terminal_tasks_are_pruned_to_the_retention_cap() {
        // MISSION-143: a session that finishes many tasks must not grow the
        // task map without bound. Spawn far past the cap, waiting each to
        // finish (each finish prunes), then assert only the newest terminal
        // tasks survive and the oldest are gone.
        let (manager, _seen) = collecting_manager();
        let mut ids = Vec::new();
        for _ in 0..(MAX_TERMINAL_TASKS + 20) {
            let id = manager.spawn(
                TaskKind::ImportFile,
                "fill".to_string(),
                |_reporter| async move { Ok(json!(null)) },
            );
            wait_terminal(&manager, &id).await;
            ids.push(id);
        }

        let remaining = manager.list();
        assert_eq!(
            remaining.len(),
            MAX_TERMINAL_TASKS,
            "only the newest terminal tasks survive the cap"
        );
        // The newest spawned task must still be present; the very first must
        // have been evicted.
        assert!(
            manager.get(ids.last().unwrap()).is_some(),
            "the newest task survives pruning"
        );
        assert!(
            manager.get(&ids[0]).is_none(),
            "the oldest task was evicted"
        );
        // `list` is newest-first, so the head is the most recently finished.
        assert_eq!(remaining[0].id, *ids.last().unwrap());
    }

    #[tokio::test]
    async fn running_tasks_are_never_pruned() {
        // MISSION-143: the retention cap applies to *terminal* tasks only — a
        // running task must never be evicted to make room.
        let (manager, _seen) = collecting_manager();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel::<()>();
        let running_id = manager.spawn(
            TaskKind::ImportFile,
            "long runner".to_string(),
            move |_reporter| async move {
                let _ = release_rx.await;
                Ok(json!(null))
            },
        );

        // Fill well past the cap with fast tasks; the long runner is still
        // alive the whole time.
        for _ in 0..(MAX_TERMINAL_TASKS + 10) {
            let id = manager.spawn(
                TaskKind::ImportFile,
                "fill".to_string(),
                |_reporter| async move { Ok(json!(null)) },
            );
            wait_terminal(&manager, &id).await;
        }
        assert!(
            manager.get(&running_id).is_some(),
            "a running task survives pruning"
        );

        release_tx.send(()).ok();
        wait_terminal(&manager, &running_id).await;
    }
}
