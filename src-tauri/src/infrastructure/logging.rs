//! Logging setup: `tracing` with a rolling daily file writer plus stdout.
//!
//! Policy (spec §38): never log secrets, API keys, tokens, or user data.
//! Errors surfaced to the UI via [`crate::AppError`] are the public surface;
//! keep internal detail behind `debug`/`trace` levels so it can be hidden in
//! production builds.

use std::{
    path::Path,
    sync::{Once, OnceLock},
};

use tracing_appender::non_blocking;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

static INIT: Once = Once::new();
static LOG_GUARD: OnceLock<non_blocking::WorkerGuard> = OnceLock::new();

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
        let _ = LOG_GUARD.set(guard);

        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("mylore=info,tauri=warn"));

        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().with_writer(file_writer).with_ansi(false))
            .with(fmt::layer().with_writer(std::io::stdout))
            .init();
    });
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

        let _ = std::fs::remove_dir_all(&dir);
    }
}
