//! SQLite pool as managed application state (MISSION-011).
//!
//! `connect` opens (creating if missing) a file-backed database with the
//! runtime pragmas applied per connection; `integrity_check` verifies the
//! database is healthy at startup; `init` runs both.

use std::{path::Path, time::Duration};

use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous,
};

use crate::error::AppError;

/// How long a writer waits for a busy database before failing (ms).
pub const BUSY_TIMEOUT_MS: u64 = 5000;
/// Maximum number of pooled connections.
const MAX_CONNECTIONS: u32 = 5;

/// Open (creating if missing) the SQLite database with FK, WAL and
/// busy-timeout pragmas applied on every connection.
pub async fn connect(db_path: &Path) -> Result<SqlitePool, AppError> {
    let options = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_millis(BUSY_TIMEOUT_MS))
        .synchronous(SqliteSynchronous::Normal);

    SqlitePoolOptions::new()
        .max_connections(MAX_CONNECTIONS)
        .connect_with(options)
        .await
        .map_err(|e| AppError::internal(format!("failed to open database: {e}")))
}

/// Run `PRAGMA integrity_check`; returns `Ok(())` when the database is healthy.
pub async fn integrity_check(pool: &SqlitePool) -> Result<(), AppError> {
    let (result,): (String,) = sqlx::query_as("PRAGMA integrity_check")
        .fetch_one(pool)
        .await?;
    if result == "ok" {
        Ok(())
    } else {
        Err(AppError::internal(format!(
            "database integrity check failed: {result}"
        )))
    }
}

/// Open the database and verify integrity — the startup entry point.
pub async fn init(db_path: &Path) -> Result<SqlitePool, AppError> {
    let pool = connect(db_path).await?;
    integrity_check(&pool).await?;
    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Unique temp dir per test process; returns a fresh db path.
    fn temp_db_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("mylore-db-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join(name);
        cleanup_files(&path);
        path
    }

    fn cleanup_files(path: &Path) {
        let base = path.display().to_string();
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{base}{suffix}"));
        }
    }

    #[tokio::test]
    async fn connect_creates_database_and_applies_pragmas() {
        let path = temp_db_path("pragma.db");
        let pool = connect(&path).await.expect("connect");

        let (foreign_keys,): (i64,) = sqlx::query_as("PRAGMA foreign_keys")
            .fetch_one(&pool)
            .await
            .expect("foreign_keys pragma");
        assert_eq!(foreign_keys, 1, "foreign keys must be enforced");

        let (busy_timeout,): (i64,) = sqlx::query_as("PRAGMA busy_timeout")
            .fetch_one(&pool)
            .await
            .expect("busy_timeout pragma");
        assert!(
            busy_timeout >= BUSY_TIMEOUT_MS as i64,
            "busy_timeout should be at least {BUSY_TIMEOUT_MS}ms, got {busy_timeout}"
        );

        let (journal_mode,): (String,) = sqlx::query_as("PRAGMA journal_mode")
            .fetch_one(&pool)
            .await
            .expect("journal_mode pragma");
        assert_eq!(journal_mode, "wal", "journal mode should be WAL");

        assert!(path.exists(), "database file should be created");
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn integrity_check_passes_on_fresh_database() {
        let path = temp_db_path("fresh.db");
        let pool = init(&path).await.expect("init");
        assert!(path.exists());
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn startup_rejects_corrupt_database() {
        let path = temp_db_path("corrupt.db");
        {
            let pool = init(&path).await.expect("create healthy db first");
            pool.close().await;
        }
        std::fs::write(&path, b"garbage".repeat(1024)).expect("corrupt the db file");

        let result = init(&path).await;
        assert!(result.is_err(), "startup must reject a corrupt database");
        cleanup_files(&path);
    }
}
