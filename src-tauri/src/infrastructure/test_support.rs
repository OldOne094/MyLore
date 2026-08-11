//! Test harness for `cargo test`.
//!
//! - `in_memory_pool` — in-memory SQLite with pragmas (unit tests; single
//!   connection so every query hits the same database).
//! - `temp_db_path` / `cleanup_files` — isolated file-backed DB paths per test.
//! - `migrated_pool` — a fully-migrated file-backed pool via `db::init`.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::path::{Path, PathBuf};
use std::{str::FromStr, time::Duration};

/// Create a pool over an in-memory SQLite database with pragmas applied.
///
/// `max_connections(1)` keeps a single connection so every query in a test
/// hits the same in-memory database.
pub async fn in_memory_pool() -> SqlitePool {
    let options = SqliteConnectOptions::from_str("sqlite::memory:")
        .expect("valid sqlite memory uri")
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5))
        .pragma("recursive_triggers", "ON");

    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("failed to open in-memory database")
}

/// Unique temp dir per test process; returns a fresh db path.
pub fn temp_db_path(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mylore-db-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join(name);
    cleanup_files(&path);
    path
}

/// Remove a database file and its WAL/SHM sidecars.
pub fn cleanup_files(path: &Path) {
    let base = path.display().to_string();
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{base}{suffix}"));
    }
}

/// Open a fully-migrated file-backed pool. Returns the pool and its path.
pub async fn migrated_pool(name: &str) -> (SqlitePool, PathBuf) {
    let path = temp_db_path(name);
    let pool = crate::infrastructure::db::init(&path)
        .await
        .expect("init migrated database");
    (pool, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn harness_roundtrips_a_row() {
        let pool = in_memory_pool().await;

        sqlx::query("CREATE TABLE sample (id INTEGER PRIMARY KEY, note TEXT NOT NULL)")
            .execute(&pool)
            .await
            .expect("create table");

        sqlx::query("INSERT INTO sample (note) VALUES (?)")
            .bind("hello from cargo test")
            .execute(&pool)
            .await
            .expect("insert");

        let (id, note): (i64, String) = sqlx::query_as("SELECT id, note FROM sample LIMIT 1")
            .fetch_one(&pool)
            .await
            .expect("select");

        assert_eq!(id, 1);
        assert_eq!(note, "hello from cargo test");
    }

    #[tokio::test]
    async fn foreign_keys_are_enforced() {
        let pool = in_memory_pool().await;

        sqlx::query("CREATE TABLE parent (id INTEGER PRIMARY KEY)")
            .execute(&pool)
            .await
            .expect("create parent");

        sqlx::query(
            "CREATE TABLE child (
                 id INTEGER PRIMARY KEY,
                 parent_id INTEGER NOT NULL REFERENCES parent(id)
             )",
        )
        .execute(&pool)
        .await
        .expect("create child");

        let result = sqlx::query("INSERT INTO child (parent_id) VALUES (999)")
            .execute(&pool)
            .await;

        assert!(result.is_err(), "foreign key violation should be rejected");
    }
}
