//! FTS5 search index helpers (MISSION-018).
//!
//! The index is kept fresh by triggers (0007_media_fts.sql), so under normal
//! operation repositories never touch it. `rebuild` exists for repair and
//! migration safety: it wipes both contentless tables and repopulates them
//! from `v_media_fts_source`, matching the migration's backfill.

use sqlx::sqlite::SqlitePool;

use crate::error::AppError;

/// Rebuild the FTS index from the media tables.
///
/// Wipes both FTS5 tables, then re-inserts the full document for each media.
/// Runs in one transaction so a failure leaves the old index intact.
pub async fn rebuild(pool: &SqlitePool) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;

    sqlx::query("DELETE FROM media_fts")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM media_fts_cjk")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM media_fts_cjk")
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "INSERT INTO media_fts(rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids)
         SELECT rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids FROM v_media_fts_source",
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO media_fts_cjk(rowid, cjk)
         SELECT rowid, cjk FROM v_media_fts_source",
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::test_support::{cleanup_files, migrated_pool};

    async fn insert_media(pool: &SqlitePool, id: &str, title: &str) {
        sqlx::query(
            "INSERT INTO media (id, content_type, title_main, synopsis, created_at, updated_at)
             VALUES (?, 'novel', ?, ?, '2026-01-01', '2026-01-01')",
        )
        .bind(id)
        .bind(title)
        .bind("placeholder synopsis")
        .execute(pool)
        .await
        .expect("insert media");
    }

    /// Count rows in a contentless FTS table.
    async fn fts_count(pool: &SqlitePool, table: &str) -> i64 {
        let (count,): (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {table}"))
            .fetch_one(pool)
            .await
            .unwrap();
        count
    }

    #[tokio::test]
    async fn fts_tables_views_and_triggers_created() {
        let (pool, path) = migrated_pool("fts_schema.db").await;

        for table in ["media_fts", "media_fts_cjk"] {
            let (found,): (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?",
            )
            .bind(table)
            .fetch_one(&pool)
            .await
            .expect("check fts table");
            assert_eq!(found, 1, "{table} should exist after migration");
        }

        let (view,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'view' AND name = 'v_media_fts_source'",
        )
        .fetch_one(&pool)
        .await
        .expect("check view");
        assert_eq!(view, 1, "v_media_fts_source view should exist");

        let (triggers,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'trigger' AND name LIKE 'trg_%_fts_%'",
        )
        .fetch_one(&pool)
        .await
        .expect("count fts triggers");
        assert_eq!(triggers, 21, "all 21 FTS refresh triggers should exist");

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn inserting_media_populates_both_indexes() {
        let (pool, path) = migrated_pool("fts_insert.db").await;
        insert_media(&pool, "m-1", "Sword of the Dawn").await;

        assert_eq!(fts_count(&pool, "media_fts").await, 1);
        assert_eq!(fts_count(&pool, "media_fts_cjk").await, 1);

        // unicode61 tokenizer: whole-word match.
        let (hits,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM media_fts WHERE media_fts MATCH 'dawn'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(hits, 1, "title token should be searchable");

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn cjk_trigram_indexes_substrings() {
        let (pool, path) = migrated_pool("fts_cjk.db").await;
        insert_media(&pool, "m-1", "魔法使いの夜").await;

        // trigram tokenizer: any 3-char substring matches.
        let (hits,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM media_fts_cjk WHERE media_fts_cjk MATCH '使いの'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(hits, 1, "3-char CJK substring should match");

        // Query tokens shorter than 3 chars are ignored by trigram — verify a
        // 2-char query returns nothing rather than crashing.
        let (short,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM media_fts_cjk WHERE media_fts_cjk MATCH '使い'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(short, 0, "sub-3-char queries match nothing");

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn arabic_diacritics_and_variants_folded() {
        let (pool, path) = migrated_pool("fts_arabic.db").await;
        // Store with diacritics and the hamza variant forms.
        sqlx::query(
            "INSERT INTO media (id, content_type, title_main, created_at, updated_at)
             VALUES ('m-1', 'novel', 'عَبْقَرِيَّةٌ', '2026-01-01', '2026-01-01')",
        )
        .execute(&pool)
        .await
        .expect("insert arabic media");

        // Index-time folding strips diacritics and maps تاء مربوطة -> هاء, so
        // 'عَبْقَرِيَّةٌ' is indexed as 'عبقريه'. Query-time folding must apply
        // the same fold (migration 0007; app-side at MATCH time), so the query
        // below is the folded form.
        let (hits,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM media_fts WHERE media_fts MATCH 'عبقريه'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            hits, 1,
            "Arabic diacritics should be stripped at index time"
        );

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn alt_title_change_refreshes_index() {
        let (pool, path) = migrated_pool("fts_alt.db").await;
        insert_media(&pool, "m-1", "Original").await;
        sqlx::query(
            "INSERT INTO media_alt_title (media_id, lang, title) VALUES ('m-1', 'ja', '別タイトル')",
        )
        .execute(&pool)
        .await
        .unwrap();

        let (hits,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM media_fts_cjk WHERE media_fts_cjk MATCH '別タイトル'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(hits, 1, "alt title should become searchable on insert");

        sqlx::query("DELETE FROM media_alt_title WHERE media_id = 'm-1' AND lang = 'ja'")
            .execute(&pool)
            .await
            .unwrap();

        let (after,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM media_fts_cjk WHERE media_fts_cjk MATCH '別タイトル'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(after, 0, "deleting an alt title should refresh the index");

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn deleting_media_removes_from_index() {
        let (pool, path) = migrated_pool("fts_delete.db").await;
        insert_media(&pool, "m-1", "Gone Tomorrow").await;
        assert_eq!(fts_count(&pool, "media_fts").await, 1);

        sqlx::query("DELETE FROM media WHERE id = 'm-1'")
            .execute(&pool)
            .await
            .unwrap();

        assert_eq!(fts_count(&pool, "media_fts").await, 0);
        assert_eq!(fts_count(&pool, "media_fts_cjk").await, 0);

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn cascade_delete_refreshes_index_via_recursive_triggers() {
        let (pool, path) = migrated_pool("fts_cascade.db").await;
        insert_media(&pool, "m-1", "Unread").await;
        sqlx::query(
            "INSERT INTO person (id, name, role) VALUES ('p-1', 'Famous Author', 'author')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO media_person (media_id, person_id) VALUES ('m-1', 'p-1')")
            .execute(&pool)
            .await
            .unwrap();

        let (hits,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM media_fts WHERE media_fts MATCH 'author'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(hits, 1, "person name should be searchable");

        // Deleting the person cascades the media_person link; the FTS trigger
        // on media_person must re-fire and reindex media_fts.
        sqlx::query("DELETE FROM person WHERE id = 'p-1'")
            .execute(&pool)
            .await
            .unwrap();

        let (after,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM media_fts WHERE media_fts MATCH 'author'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            after, 0,
            "cascade deletion should refresh the FTS index via recursive triggers"
        );

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn updating_media_title_refreshes_index() {
        let (pool, path) = migrated_pool("fts_update.db").await;
        insert_media(&pool, "m-1", "Old Title").await;

        let (before,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM media_fts WHERE media_fts MATCH 'new'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(before, 0);

        sqlx::query("UPDATE media SET title_main = 'New Title' WHERE id = 'm-1'")
            .execute(&pool)
            .await
            .unwrap();

        let (after,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM media_fts WHERE media_fts MATCH 'new'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(after, 1, "updating media should refresh the index");

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn rebuild_restores_index_after_manual_wipes() {
        let (pool, path) = migrated_pool("fts_rebuild.db").await;
        insert_media(&pool, "m-1", "Recover Me").await;

        // Simulate a corrupted/missing index.
        sqlx::query("DELETE FROM media_fts")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(fts_count(&pool, "media_fts").await, 0);

        rebuild(&pool).await.expect("rebuild");

        assert_eq!(fts_count(&pool, "media_fts").await, 1);
        assert_eq!(fts_count(&pool, "media_fts_cjk").await, 1);
        let (hits,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM media_fts WHERE media_fts MATCH 'recover'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(hits, 1, "rebuilt index should be searchable");

        pool.close().await;
        cleanup_files(&path);
    }
}
