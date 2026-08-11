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

/// Open (creating if missing) the SQLite database with FK, WAL,
/// busy-timeout and recursive-trigger pragmas applied on every connection.
///
/// `recursive_triggers = ON` makes `ON DELETE CASCADE` deletions re-fire the
/// triggers of child tables, so e.g. deleting media refreshes the FTS index
/// (0007_media_fts.sql) even when the row is removed by a cascade.
pub async fn connect(db_path: &Path) -> Result<SqlitePool, AppError> {
    let options = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_millis(BUSY_TIMEOUT_MS))
        .synchronous(SqliteSynchronous::Normal)
        .pragma("recursive_triggers", "ON");

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

    #[tokio::test]
    async fn tracking_status_node_progress_tables_created() {
        let (pool, path) = migrated_pool("tracking_schema.db").await;

        for table in ["tracking", "status", "node_progress"] {
            let (found,): (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?",
            )
            .bind(table)
            .fetch_one(&pool)
            .await
            .expect("check table exists");
            assert_eq!(found, 1, "{table} table should exist after migration");
        }

        for index in ["idx_tracking_core_status", "idx_tracking_updated_at"] {
            let (found,): (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?",
            )
            .bind(index)
            .fetch_one(&pool)
            .await
            .expect("check index");
            assert_eq!(found, 1, "{index} should exist after migration");
        }

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn core_statuses_are_seeded() {
        let (pool, path) = migrated_pool("status_seeds.db").await;

        let (system_count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM status WHERE is_system = 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(system_count, 7, "all seven core statuses should be seeded");

        for (id, bucket) in [
            ("planned", "planned"),
            ("in_progress", "in_progress"),
            ("completed", "completed"),
            ("on_hold", "on_hold"),
            ("dropped", "dropped"),
            ("repeat", "repeat"),
            ("wishlist", "wishlist"),
        ] {
            let (matches,): (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM status WHERE id = ? AND bucket = ? AND is_system = 1",
            )
            .bind(id)
            .bind(bucket)
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(matches, 1, "core status {id} should be seeded");
        }

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn tracking_checks_and_cascade_enforced() {
        let (pool, path) = migrated_pool("tracking_checks.db").await;
        insert_media(&pool, "m-1").await;

        sqlx::query(
            "INSERT INTO tracking (media_id, core_status, updated_at)
             VALUES ('m-1', 'in_progress', '2026-01-01')",
        )
        .execute(&pool)
        .await
        .expect("valid core_status");

        let bad_status = sqlx::query(
            "INSERT INTO tracking (media_id, core_status, updated_at)
             VALUES ('m-1', 'watching', '2026-01-01')",
        )
        .execute(&pool)
        .await;
        assert!(
            bad_status.is_err(),
            "CHECK must reject an unknown core_status"
        );

        insert_media(&pool, "m-2").await;
        let negative_repeat = sqlx::query(
            "INSERT INTO tracking (media_id, core_status, repeat_count, updated_at)
             VALUES ('m-2', 'planned', -1, '2026-01-01')",
        )
        .execute(&pool)
        .await;
        assert!(
            negative_repeat.is_err(),
            "CHECK(repeat_count >= 0) must reject negative repeat count"
        );

        sqlx::query(
            "INSERT INTO tracking (media_id, core_status, repeat_count, updated_at)
             VALUES ('m-2', 'planned', 2, '2026-01-01')",
        )
        .execute(&pool)
        .await
        .expect("insert valid tracking row for m-2");

        let missing_media = sqlx::query(
            "INSERT INTO tracking (media_id, core_status, updated_at)
             VALUES ('nope', 'planned', '2026-01-01')",
        )
        .execute(&pool)
        .await;
        assert!(missing_media.is_err(), "FK must reject unknown media");

        sqlx::query("DELETE FROM media WHERE id = 'm-1'")
            .execute(&pool)
            .await
            .unwrap();
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracking")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1, "tracking should cascade-delete with media");

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn tracking_custom_status_fk_sets_null_on_delete() {
        let (pool, path) = migrated_pool("tracking_custom.db").await;
        insert_media(&pool, "m-1").await;
        sqlx::query(
            "INSERT INTO status (id, name, bucket) VALUES ('re-reading', 'Re-reading', 'repeat')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO tracking (media_id, core_status, custom_status_id, updated_at)
             VALUES ('m-1', 'repeat', 're-reading', '2026-01-01')",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query("DELETE FROM status WHERE id = 're-reading'")
            .execute(&pool)
            .await
            .unwrap();

        let (custom,): (Option<String>,) =
            sqlx::query_as("SELECT custom_status_id FROM tracking WHERE media_id = 'm-1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            custom, None,
            "deleting a custom status must SET NULL on tracking"
        );

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn node_progress_checks_and_cascade_enforced() {
        let (pool, path) = migrated_pool("node_progress.db").await;
        insert_media(&pool, "m-1").await;
        sqlx::query(
            "INSERT INTO content_node (id, media_id, kind, position, created_at)
             VALUES ('n-1', 'm-1', 'chapter', 1, '2026-01-01')",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO node_progress (node_id, state, rating, updated_at)
             VALUES ('n-1', 'read', 8, '2026-01-01')",
        )
        .execute(&pool)
        .await
        .expect("valid node progress");

        let bad_state = sqlx::query(
            "INSERT INTO node_progress (node_id, state, updated_at)
             VALUES ('n-1', 'finished', '2026-01-01')",
        )
        .execute(&pool)
        .await;
        assert!(bad_state.is_err(), "CHECK must reject an unknown state");

        insert_media(&pool, "m-2").await;
        sqlx::query(
            "INSERT INTO content_node (id, media_id, kind, position, created_at)
             VALUES ('n-2', 'm-2', 'chapter', 1, '2026-01-01')",
        )
        .execute(&pool)
        .await
        .unwrap();
        for rating in [0i64, 11] {
            let out_of_range = sqlx::query(
                "INSERT INTO node_progress (node_id, state, rating, updated_at)
                 VALUES ('n-2', 'read', ?, '2026-01-01')",
            )
            .bind(rating)
            .execute(&pool)
            .await;
            assert!(out_of_range.is_err(), "rating CHECK must reject {rating}");
        }

        let dup = sqlx::query(
            "INSERT INTO node_progress (node_id, state, updated_at)
             VALUES ('n-1', 'partial', '2026-01-01')",
        )
        .execute(&pool)
        .await;
        assert!(dup.is_err(), "node_progress PK is per node");

        sqlx::query("DELETE FROM content_node WHERE id = 'n-2'")
            .execute(&pool)
            .await
            .unwrap();
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM node_progress")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            count, 1,
            "node_progress should cascade-delete with its node"
        );

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn user_aggregate_tables_created() {
        let (pool, path) = migrated_pool("user_agg_schema.db").await;

        for table in [
            "review",
            "collection",
            "collection_member",
            "asset",
            "activity",
            "trash",
            "settings",
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

        let (asset_columns,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM pragma_table_info('media') WHERE name IN ('cover_asset_id', 'banner_asset_id')",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            asset_columns, 2,
            "media should have the deferred asset columns"
        );

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn media_cover_asset_fk_sets_null_on_delete() {
        let (pool, path) = migrated_pool("media_asset_fk.db").await;
        insert_media(&pool, "m-1").await;
        sqlx::query(
            "INSERT INTO asset (id, kind, created_at) VALUES ('a-1', 'cover', '2026-01-01')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("UPDATE media SET cover_asset_id = 'a-1' WHERE id = 'm-1'")
            .execute(&pool)
            .await
            .unwrap();

        sqlx::query("DELETE FROM asset WHERE id = 'a-1'")
            .execute(&pool)
            .await
            .unwrap();

        let (cover,): (Option<String>,) =
            sqlx::query_as("SELECT cover_asset_id FROM media WHERE id = 'm-1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            cover, None,
            "deleting an asset must SET NULL on media.cover_asset_id"
        );

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn review_checks_and_cascade() {
        let (pool, path) = migrated_pool("review.db").await;
        insert_media(&pool, "m-1").await;
        insert_media(&pool, "m-2").await;

        sqlx::query(
            "INSERT INTO review (media_id, rating, favorite, created_at, updated_at)
             VALUES ('m-1', 8, 1, '2026-01-01', '2026-01-01')",
        )
        .execute(&pool)
        .await
        .expect("valid review");

        for rating in [0i64, 11] {
            let out_of_range = sqlx::query(
                "INSERT INTO review (media_id, rating, created_at, updated_at)
                 VALUES ('m-2', ?, '2026-01-01', '2026-01-01')",
            )
            .bind(rating)
            .execute(&pool)
            .await;
            assert!(out_of_range.is_err(), "rating CHECK must reject {rating}");
        }

        let bad_favorite = sqlx::query(
            "INSERT INTO review (media_id, favorite, created_at, updated_at)
             VALUES ('m-2', -1, '2026-01-01', '2026-01-01')",
        )
        .execute(&pool)
        .await;
        assert!(bad_favorite.is_err(), "favorite CHECK must reject -1");

        sqlx::query("DELETE FROM media WHERE id = 'm-1'")
            .execute(&pool)
            .await
            .unwrap();
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM review")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0, "review should cascade-delete with media");

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn collection_members_cascade_both_ways() {
        let (pool, path) = migrated_pool("collection.db").await;
        insert_media(&pool, "m-1").await;
        insert_media(&pool, "m-2").await;
        sqlx::query(
            "INSERT INTO collection (id, name, created_at) VALUES ('c-1', 'Reading', '2026-01-01')",
        )
        .execute(&pool)
        .await
        .unwrap();
        for media_id in ["m-1", "m-2"] {
            sqlx::query(
                "INSERT INTO collection_member (collection_id, media_id, added_at) VALUES ('c-1', ?, '2026-01-01')",
            )
            .bind(media_id)
            .execute(&pool)
            .await
            .unwrap();
        }

        sqlx::query("DELETE FROM media WHERE id = 'm-2'")
            .execute(&pool)
            .await
            .unwrap();
        let (after_media_delete,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM collection_member WHERE collection_id = 'c-1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            after_media_delete, 1,
            "collection_member should cascade-delete with media"
        );

        sqlx::query("DELETE FROM collection WHERE id = 'c-1'")
            .execute(&pool)
            .await
            .unwrap();
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM collection_member")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            count, 0,
            "collection_member should cascade-delete with collection"
        );

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn activity_checks_and_cascade() {
        let (pool, path) = migrated_pool("activity.db").await;
        insert_media(&pool, "m-1").await;

        sqlx::query("INSERT INTO activity (id, media_id, kind, created_at) VALUES ('a-1', 'm-1', 'added', '2026-01-01')")
            .execute(&pool)
            .await
            .expect("valid activity kind");

        let bad_kind = sqlx::query(
            "INSERT INTO activity (id, kind, created_at) VALUES ('a-2', 'watched', '2026-01-01')",
        )
        .execute(&pool)
        .await;
        assert!(
            bad_kind.is_err(),
            "activity kind CHECK must reject unknown values"
        );

        sqlx::query("DELETE FROM media WHERE id = 'm-1'")
            .execute(&pool)
            .await
            .unwrap();
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM activity")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0, "activity should cascade-delete with media");

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn trash_kind_check_and_settings_roundtrip() {
        let (pool, path) = migrated_pool("trash_settings.db").await;

        let bad_kind = sqlx::query(
            "INSERT INTO trash (id, kind, payload, deleted_at) VALUES ('t-1', 'file', '{}', '2026-01-01')",
        )
        .execute(&pool)
        .await;
        assert!(
            bad_kind.is_err(),
            "trash kind CHECK must reject unknown values"
        );

        sqlx::query(
            "INSERT INTO trash (id, kind, payload, deleted_at) VALUES ('t-2', 'media', '{}', '2026-01-01')",
        )
        .execute(&pool)
        .await
        .expect("valid trash row");

        sqlx::query("INSERT INTO settings (key, value) VALUES ('ui.language', 'ar')")
            .execute(&pool)
            .await
            .unwrap();
        let (value,): (String,) =
            sqlx::query_as("SELECT value FROM settings WHERE key = 'ui.language'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(value, "ar", "settings should roundtrip values");

        pool.close().await;
        cleanup_files(&path);
    }
}
