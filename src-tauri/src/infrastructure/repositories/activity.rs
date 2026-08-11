//! Activity-log repository (MISSION-019). Append-only user events.

use sqlx::sqlite::{SqlitePool, SqliteRow};
use sqlx::Row;

use crate::error::AppError;

/// An activity-log entry.
#[derive(Debug, Clone)]
pub struct ActivityRecord {
    pub id: String,
    pub media_id: Option<String>,
    pub node_id: Option<String>,
    pub kind: String,
    pub meta: Option<String>,
    pub created_at: String,
}

/// Append an activity entry.
pub async fn log(pool: &SqlitePool, a: &ActivityRecord) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO activity (id, media_id, node_id, kind, meta, created_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&a.id)
    .bind(&a.media_id)
    .bind(&a.node_id)
    .bind(&a.kind)
    .bind(&a.meta)
    .bind(&a.created_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Most recent entries, newest first.
pub async fn list_recent(pool: &SqlitePool, limit: u32) -> Result<Vec<ActivityRecord>, AppError> {
    let rows = sqlx::query(
        "SELECT id, media_id, node_id, kind, meta, created_at
         FROM activity ORDER BY created_at DESC, id LIMIT ?",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(row_to_activity).collect())
}

/// Entries for one media, newest first.
pub async fn list_for_media(
    pool: &SqlitePool,
    media_id: &str,
    limit: u32,
) -> Result<Vec<ActivityRecord>, AppError> {
    let rows = sqlx::query(
        "SELECT id, media_id, node_id, kind, meta, created_at
         FROM activity WHERE media_id = ? ORDER BY created_at DESC, id LIMIT ?",
    )
    .bind(media_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(row_to_activity).collect())
}

fn row_to_activity(row: SqliteRow) -> ActivityRecord {
    let get = |idx: usize| -> Option<String> { row.get(idx) };
    ActivityRecord {
        id: get(0).expect("id"),
        media_id: get(1),
        node_id: get(2),
        kind: get(3).expect("kind"),
        meta: get(4),
        created_at: get(5).expect("created_at"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::test_support::{cleanup_files, migrated_pool};

    fn activity(id: &str, media_id: Option<&str>, kind: &str, created_at: &str) -> ActivityRecord {
        ActivityRecord {
            id: id.to_string(),
            media_id: media_id.map(str::to_string),
            node_id: None,
            kind: kind.to_string(),
            meta: None,
            created_at: created_at.to_string(),
        }
    }

    #[tokio::test]
    async fn logs_and_lists_newest_first() {
        let (pool, path) = migrated_pool("activity_repo.db").await;
        sqlx::query(
            "INSERT INTO media (id, content_type, title_main, created_at, updated_at)
             VALUES ('m-1', 'novel', 'Title', '2026-01-01', '2026-01-01')",
        )
        .execute(&pool)
        .await
        .expect("seed media");

        log(&pool, &activity("a-1", Some("m-1"), "added", "2026-01-01"))
            .await
            .expect("log");
        log(
            &pool,
            &activity("a-2", Some("m-1"), "started", "2026-01-02"),
        )
        .await
        .expect("log");
        log(&pool, &activity("a-3", None, "imported", "2026-01-03"))
            .await
            .expect("log");

        let recent = list_recent(&pool, 10).await.expect("recent");
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].id, "a-3", "newest first");

        let for_media = list_for_media(&pool, "m-1", 10).await.expect("for media");
        assert_eq!(for_media.len(), 2);
        assert_eq!(for_media[0].id, "a-2");

        let capped = list_recent(&pool, 2).await.expect("capped");
        assert_eq!(capped.len(), 2);
        pool.close().await;
        cleanup_files(&path);
    }
}
