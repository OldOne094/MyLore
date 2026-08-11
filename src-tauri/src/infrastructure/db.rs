//! SQLite pool as managed application state (MISSION-011/012).
//!
//! `connect` opens (creating if missing) a file-backed database with the
//! runtime pragmas applied per connection; `integrity_check` verifies the
//! database is healthy at startup; `migrate` applies pending versioned
//! migrations (each wrapped in its own transaction by sqlx); `init` runs all
//! three.

use std::{path::Path, time::Duration};

use sqlx::migrate::Migrator;
use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous,
};

use crate::error::AppError;

/// Versioned migrations, embedded at compile time from `migrations/`.
static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

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

/// Apply pending migrations. sqlx runs each migration SQL and its bookkeeping
/// in a single transaction (sqlx-sqlite 0.8.6 `migrate.rs`), so an applied
/// migration can never be half-executed or re-run.
pub async fn migrate(pool: &SqlitePool) -> Result<(), AppError> {
    MIGRATOR.run(pool).await?;
    Ok(())
}

/// Open the database, verify integrity and apply migrations — the startup
/// entry point.
pub async fn init(db_path: &Path) -> Result<SqlitePool, AppError> {
    let pool = connect(db_path).await?;
    integrity_check(&pool).await?;
    migrate(&pool).await?;
    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::test_support::{cleanup_files, migrated_pool, temp_db_path};

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
        // A file that was never a valid SQLite database must be rejected at
        // startup. Using a fresh path avoids WAL/SHM sidecar files, which on
        // Windows can still be held open briefly after `pool.close()` and
        // would let SQLite recover a schema from the WAL.
        let path = temp_db_path("corrupt.db");
        std::fs::write(&path, b"garbage".repeat(1024)).expect("write corrupt db file");

        let result = init(&path).await;
        assert!(result.is_err(), "startup must reject a corrupt database");
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn migrate_applies_pending_migrations() {
        let path = temp_db_path("migrate.db");
        let pool = init(&path).await.expect("init");

        let (applied,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM _sqlx_migrations WHERE success = TRUE")
                .fetch_one(&pool)
                .await
                .expect("count applied migrations");
        let (version,): (i64,) = sqlx::query_as("SELECT MAX(version) FROM _sqlx_migrations")
            .fetch_one(&pool)
            .await
            .expect("max version");
        assert_eq!(
            applied, version,
            "every migration must have applied cleanly"
        );
        assert!(version >= 3, "expected at least the 3 known migrations");

        let (has_app_meta,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'app_meta'",
        )
        .fetch_one(&pool)
        .await
        .expect("check app_meta table");
        assert_eq!(
            has_app_meta, 1,
            "app_meta table should exist after migration"
        );

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn migrate_is_idempotent() {
        let path = temp_db_path("idempotent.db");
        let version_after_first = {
            let pool = init(&path).await.expect("first init");
            let (version,): (i64,) = sqlx::query_as("SELECT MAX(version) FROM _sqlx_migrations")
                .fetch_one(&pool)
                .await
                .expect("max version");
            pool.close().await;
            version
        };
        {
            let pool = init(&path).await.expect("second init");
            let (version,): (i64,) = sqlx::query_as("SELECT MAX(version) FROM _sqlx_migrations")
                .fetch_one(&pool)
                .await
                .expect("max version");
            assert_eq!(
                version, version_after_first,
                "second init must not re-apply migrations"
            );
            pool.close().await;
        }
        cleanup_files(&path);
    }

    /// Insert a minimal media row for FK/cascade tests.
    async fn insert_media(pool: &SqlitePool, id: &str) {
        sqlx::query(
            "INSERT INTO media (id, content_type, title_main, created_at, updated_at)
             VALUES (?, 'novel', 'Test Title', '2026-01-01', '2026-01-01')",
        )
        .bind(id)
        .execute(pool)
        .await
        .expect("insert media");
    }

    #[tokio::test]
    async fn media_schema_tables_and_seeds_created() {
        let path = temp_db_path("media_schema.db");
        let pool = init(&path).await.expect("init");

        for table in [
            "media",
            "media_alt_title",
            "person",
            "media_person",
            "genre",
            "media_genre",
            "tag",
            "media_tag",
        ] {
            let (found,): (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?",
            )
            .bind(table)
            .fetch_one(&pool)
            .await
            .expect("check table exists");
            assert_eq!(found, 1, "{table} table should exist after migration");
        }

        let (genres,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM genre")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(genres >= 18, "core genres should be seeded, got {genres}");

        let (tags,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tag")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(tags >= 20, "domain tags should be seeded, got {tags}");

        let (has_isekai,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM tag WHERE id = 'isekai' AND scope = 'domain'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(has_isekai, 1, "seed tag 'isekai' should be a domain tag");

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn media_aggregate_cascades_on_media_delete() {
        let path = temp_db_path("media_cascade.db");
        let pool = init(&path).await.expect("init");

        insert_media(&pool, "m-1").await;
        sqlx::query("INSERT INTO media_alt_title (media_id, lang, title) VALUES ('m-1', 'ja', '元タイトル')")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO person (id, name, role) VALUES ('p-1', 'Author Name', 'author')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO media_person (media_id, person_id) VALUES ('m-1', 'p-1')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO media_genre (media_id, genre_id) VALUES ('m-1', 'fantasy')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO media_tag (media_id, tag_id) VALUES ('m-1', 'isekai')")
            .execute(&pool)
            .await
            .unwrap();

        sqlx::query("DELETE FROM media WHERE id = 'm-1'")
            .execute(&pool)
            .await
            .unwrap();

        for (sql, what) in [
            (
                "SELECT COUNT(*) FROM media_alt_title WHERE media_id = 'm-1'",
                "alt titles",
            ),
            (
                "SELECT COUNT(*) FROM media_person WHERE media_id = 'm-1'",
                "media_person links",
            ),
            (
                "SELECT COUNT(*) FROM media_genre WHERE media_id = 'm-1'",
                "media_genre links",
            ),
            (
                "SELECT COUNT(*) FROM media_tag WHERE media_id = 'm-1'",
                "media_tag links",
            ),
        ] {
            let (n,): (i64,) = sqlx::query_as(sql).fetch_one(&pool).await.unwrap();
            assert_eq!(n, 0, "{what} should cascade-delete with media");
        }

        for (sql, what) in [
            ("SELECT COUNT(*) FROM person WHERE id = 'p-1'", "person"),
            ("SELECT COUNT(*) FROM genre WHERE id = 'fantasy'", "genre"),
            ("SELECT COUNT(*) FROM tag WHERE id = 'isekai'", "tag"),
        ] {
            let (n,): (i64,) = sqlx::query_as(sql).fetch_one(&pool).await.unwrap();
            assert_eq!(n, 1, "{what} should survive media deletion");
        }

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn media_person_cascade_removes_link_on_person_delete() {
        let path = temp_db_path("person_cascade.db");
        let pool = init(&path).await.expect("init");

        insert_media(&pool, "m-1").await;
        sqlx::query("INSERT INTO person (id, name, role) VALUES ('p-1', 'Artist Name', 'artist')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO media_person (media_id, person_id) VALUES ('m-1', 'p-1')")
            .execute(&pool)
            .await
            .unwrap();

        sqlx::query("DELETE FROM person WHERE id = 'p-1'")
            .execute(&pool)
            .await
            .unwrap();

        let (links,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM media_person WHERE media_id = 'm-1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            links, 0,
            "media_person link should cascade-delete with person"
        );

        let (media,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM media WHERE id = 'm-1'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(media, 1, "media should survive person deletion");

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn media_foreign_keys_are_enforced() {
        let path = temp_db_path("media_fk.db");
        let pool = init(&path).await.expect("init");

        let cases = [
            "INSERT INTO media_alt_title (media_id, lang, title) VALUES ('missing', 'en', 'X')",
            "INSERT INTO media_person (media_id, person_id) VALUES ('missing', 'p-1')",
            "INSERT INTO media_genre (media_id, genre_id) VALUES ('missing', 'fantasy')",
            "INSERT INTO media_tag (media_id, tag_id) VALUES ('missing', 'isekai')",
        ];
        for sql in cases {
            let result = sqlx::query(sql).execute(&pool).await;
            assert!(result.is_err(), "FK must reject {sql}");
        }

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn external_id_and_relation_tables_created() {
        let (pool, path) = migrated_pool("identity_schema.db").await;

        for table in ["media_external_id", "media_relation"] {
            let (found,): (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?",
            )
            .bind(table)
            .fetch_one(&pool)
            .await
            .expect("check table exists");
            assert_eq!(found, 1, "{table} table should exist after migration");
        }

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn external_id_unique_constraints_enforced() {
        let (pool, path) = migrated_pool("ext_id_unique.db").await;
        insert_media(&pool, "m-1").await;
        insert_media(&pool, "m-2").await;

        sqlx::query(
            "INSERT INTO media_external_id (media_id, provider, ext_id) VALUES ('m-1', 'anilist', '123')",
        )
        .execute(&pool)
        .await
        .expect("insert external id");

        let dup_provider = sqlx::query(
            "INSERT INTO media_external_id (media_id, provider, ext_id) VALUES ('m-1', 'anilist', '456')",
        )
        .execute(&pool)
        .await;
        assert!(
            dup_provider.is_err(),
            "UNIQUE(media_id, provider) must reject a second id for the same provider"
        );

        let dup_identity = sqlx::query(
            "INSERT INTO media_external_id (media_id, provider, ext_id) VALUES ('m-2', 'anilist', '123')",
        )
        .execute(&pool)
        .await;
        assert!(
            dup_identity.is_err(),
            "PK(provider, ext_id) must reject the same provider identity for two media"
        );

        sqlx::query(
            "INSERT INTO media_external_id (media_id, provider, ext_id) VALUES ('m-1', 'tmdb', '789')",
        )
        .execute(&pool)
        .await
        .expect("a different provider for the same media is allowed");

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn external_id_cascades_with_media() {
        let (pool, path) = migrated_pool("ext_id_cascade.db").await;
        insert_media(&pool, "m-1").await;
        sqlx::query(
            "INSERT INTO media_external_id (media_id, provider, ext_id) VALUES ('m-1', 'anilist', '123')",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query("DELETE FROM media WHERE id = 'm-1'")
            .execute(&pool)
            .await
            .unwrap();

        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM media_external_id")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            count, 0,
            "media_external_id should cascade-delete with media"
        );

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn media_relation_checks_and_fks_enforced() {
        let (pool, path) = migrated_pool("relation.db").await;
        insert_media(&pool, "m-1").await;
        insert_media(&pool, "m-2").await;

        sqlx::query(
            "INSERT INTO media_relation (from_id, to_id, relation) VALUES ('m-1', 'm-2', 'sequel')",
        )
        .execute(&pool)
        .await
        .expect("a valid relation should be accepted");

        let self_relation = sqlx::query(
            "INSERT INTO media_relation (from_id, to_id, relation) VALUES ('m-1', 'm-1', 'sequel')",
        )
        .execute(&pool)
        .await;
        assert!(
            self_relation.is_err(),
            "CHECK(from_id <> to_id) must reject self-relations"
        );

        let bad_relation = sqlx::query(
            "INSERT INTO media_relation (from_id, to_id, relation) VALUES ('m-1', 'm-2', 'friends')",
        )
        .execute(&pool)
        .await;
        assert!(
            bad_relation.is_err(),
            "relation CHECK must reject unknown values"
        );

        let missing_target = sqlx::query(
            "INSERT INTO media_relation (from_id, to_id, relation) VALUES ('m-1', 'nope', 'other')",
        )
        .execute(&pool)
        .await;
        assert!(
            missing_target.is_err(),
            "FK must reject a relation to a missing media"
        );

        sqlx::query("DELETE FROM media WHERE id = 'm-2'")
            .execute(&pool)
            .await
            .unwrap();
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM media_relation")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            count, 0,
            "media_relation should cascade-delete with either endpoint"
        );

        pool.close().await;
        cleanup_files(&path);
    }
}
