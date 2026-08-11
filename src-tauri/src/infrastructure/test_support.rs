//! In-memory SQLite harness for unit tests (`cargo test`).
//!
//! `sqlite::memory:` gives each connection its own database, so the pool is
//! limited to a single connection. A real file-backed pool with WAL lands with
//! the Database milestone (M2).

use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::{str::FromStr, time::Duration};

/// Create a pool over an in-memory SQLite database with pragmas applied.
///
/// `max_connections(1)` keeps a single connection so every query in a test
/// hits the same in-memory database.
pub async fn in_memory_pool() -> SqlitePool {
    let options = SqliteConnectOptions::from_str("sqlite::memory:")
        .expect("valid sqlite memory uri")
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5));

    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("failed to open in-memory database")
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
