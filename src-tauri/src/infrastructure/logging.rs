//! Logging setup: `tracing` with a rolling daily file writer plus stdout.
//!
//! Policy (spec §38): never log secrets, API keys, tokens, or user data.
//! Errors surfaced to the UI via [`crate::AppError`] are the public surface;
//! keep internal detail behind `debug`/`trace` levels so it can be hidden in
//! production builds.

use std::{
    path::Path,
    sync::{Mutex, Once, OnceLock, PoisonError},
};

use tracing_appender::non_blocking;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

static INIT: Once = Once::new();
/// The non-blocking writer's guard. Held (rather than leaked) so
/// [`shutdown`] can flush it: dropping the guard drains the queue and stops the
/// writer thread.
static LOG_GUARD: OnceLock<Mutex<Option<non_blocking::WorkerGuard>>> = OnceLock::new();

/// Initialise tracing: rolling daily log files (max 5 retained) plus stdout.
///
/// Default level is `mylore=info,tauri=warn`; override with the `RUST_LOG`
/// env var. Safe to call more than once — only the first call takes effect.
pub fn init(log_dir: &Path) {
    INIT.call_once(|| {
        let file_appender = tracing_appender::rolling::Builder::new()
            .filename_prefix("mylore")
            .max_log_files(5)
            .rotation(tracing_appender::rolling::Rotation::DAILY)
            .build(log_dir)
            .expect("create rolling log dir");
        let (file_writer, guard) = non_blocking(file_appender);
        let slot = LOG_GUARD.get_or_init(|| Mutex::new(None));
        *slot.lock().unwrap_or_else(PoisonError::into_inner) = Some(guard);

        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("mylore=info,tauri=warn"));

        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().with_writer(file_writer).with_ansi(false))
            .with(fmt::layer().with_writer(std::io::stdout))
            .init();
    });
}

/// Flush the log and stop the file writer.
///
/// The file layer is non-blocking — a background thread with a queue — so a line
/// logged immediately before `std::process::exit` can die in that queue. Every
/// fatal path calls this after logging so the reason for the exit is on disk.
/// Safe to call more than once, and after a call the logger is inert.
pub fn shutdown() {
    let Some(slot) = LOG_GUARD.get() else {
        return;
    };
    let guard = slot.lock().unwrap_or_else(PoisonError::into_inner).take();
    drop(guard);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_is_idempotent_and_writes_log_files() {
        let dir = std::env::temp_dir().join(format!("mylore-log-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        init(&dir);
        init(&dir); // second call must not panic

        tracing::info!("log smoke test");
        std::thread::sleep(std::time::Duration::from_millis(300));

        let files = std::fs::read_dir(&dir).unwrap().count();
        assert!(files >= 1, "expected at least one log file, found {files}");

        // MISSION-118: a line logged just before a fatal exit must survive the
        // non-blocking writer's queue — fatal paths call `shutdown` first.
        tracing::error!("flush marker before a fatal exit");
        shutdown();
        shutdown(); // idempotent

        let written: String = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|entry| std::fs::read_to_string(entry.ok()?.path()).ok())
            .collect();
        assert!(
            written.contains("flush marker"),
            "shutdown must flush what was logged before the exit: {written:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
