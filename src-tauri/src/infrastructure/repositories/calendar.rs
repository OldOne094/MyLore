//! Calendar repository (MISSION-081). Two date-window queries feed the month
//! grid: content-node air/release dates and the user activity trail (joined
//! with its media row). Callers supply the window bounds — `activity.created_at`
//! is RFC3339 while `content_node.release_date` is a bare ISO date, so the two
//! windows are expressed differently by `calendar_service`.

use sqlx::sqlite::{SqlitePool, SqliteRow};
use sqlx::Row;

use crate::error::AppError;

/// A media air/release entry: one content node carrying a release date.
#[derive(Debug, Clone)]
pub struct AirDateRow {
    pub media_id: String,
    pub title: String,
    pub content_type: String,
    pub node_kind: String,
    pub node_number: Option<String>,
    pub node_position: Option<i64>,
    pub release_date: String,
}

/// One activity entry within a window, with its media title. The FK cascade
/// removes a media's activity when the media is deleted, so `title` is present
/// for every surviving row; the LEFT JOIN is defensive for legacy/unguarded
/// rows.
#[derive(Debug, Clone)]
pub struct ActivityRow {
    pub media_id: Option<String>,
    pub title: Option<String>,
    pub content_type: Option<String>,
    pub kind: String,
    pub created_at: String,
}

/// Content-node release dates within `[from, to)` (lexicographic ISO dates).
pub async fn air_dates(
    pool: &SqlitePool,
    from: &str,
    to: &str,
) -> Result<Vec<AirDateRow>, AppError> {
    let rows = sqlx::query(
        "SELECT cn.media_id, m.title_main, m.content_type, cn.kind, cn.number, cn.position, \
                cn.release_date \
         FROM content_node cn JOIN media m ON m.id = cn.media_id \
         WHERE cn.release_date IS NOT NULL AND cn.release_date >= ? AND cn.release_date < ? \
         ORDER BY cn.release_date, m.title_main, cn.position",
    )
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(row_to_air_date).collect())
}

/// Activity entries within `[from, to)` (RFC3339, lexicographic), ascending.
pub async fn activity_in_range(
    pool: &SqlitePool,
    from: &str,
    to: &str,
) -> Result<Vec<ActivityRow>, AppError> {
    let rows = sqlx::query(
        "SELECT a.media_id, m.title_main, m.content_type, a.kind, a.created_at \
         FROM activity a LEFT JOIN media m ON m.id = a.media_id \
         WHERE a.created_at >= ? AND a.created_at < ? \
         ORDER BY a.created_at, a.id",
    )
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(row_to_activity).collect())
}

fn row_to_air_date(row: SqliteRow) -> AirDateRow {
    AirDateRow {
        media_id: row.get("media_id"),
        title: row.get("title_main"),
        content_type: row.get("content_type"),
        node_kind: row.get("kind"),
        node_number: row.get("number"),
        node_position: row.get("position"),
        release_date: row.get("release_date"),
    }
}

fn row_to_activity(row: SqliteRow) -> ActivityRow {
    ActivityRow {
        media_id: row.get("media_id"),
        title: row.get("title_main"),
        content_type: row.get("content_type"),
        kind: row.get("kind"),
        created_at: row.get("created_at"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::test_support::{cleanup_files, migrated_pool};

    async fn seed_media(pool: &SqlitePool, id: &str, content_type: &str, title: &str) {
        sqlx::query(
            "INSERT INTO media (id, content_type, title_main, created_at, updated_at)
             VALUES (?, ?, ?, '2026-01-01', '2026-01-01')",
        )
        .bind(id)
        .bind(content_type)
        .bind(title)
        .execute(pool)
        .await
        .expect("seed media");
    }

    async fn seed_node(pool: &SqlitePool, id: &str, media_id: &str, release_date: &str) {
        sqlx::query(
            "INSERT INTO content_node (id, media_id, kind, position, number, release_date, created_at)
             VALUES (?, ?, 'episode', 1, ?, ?, '2026-01-01')",
        )
        .bind(id)
        .bind(media_id)
        .bind(id)
        .bind(release_date)
        .execute(pool)
        .await
        .expect("seed node");
    }

    #[tokio::test]
    async fn air_dates_returns_rows_in_the_window() {
        let (pool, path) = migrated_pool("calendar_air.db").await;
        seed_media(&pool, "m-1", "anime", "Series").await;
        seed_node(&pool, "n-1", "m-1", "2026-08-05").await;
        seed_node(&pool, "n-2", "m-1", "2026-08-12").await;

        let rows = air_dates(&pool, "2026-08-01", "2026-09-01")
            .await
            .expect("air dates");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].release_date, "2026-08-05");
        assert_eq!(rows[0].title, "Series");
        assert_eq!(rows[0].node_kind, "episode");
        assert_eq!(rows[0].node_number.as_deref(), Some("n-1"));

        let none = air_dates(&pool, "2026-09-01", "2026-10-01")
            .await
            .expect("empty window");
        assert!(none.is_empty());
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn activity_in_range_joins_media_and_survives_deleted_rows() {
        let (pool, path) = migrated_pool("calendar_activity.db").await;
        seed_media(&pool, "m-1", "novel", "Book").await;
        seed_media(&pool, "m-2", "anime", "Show").await;

        sqlx::query(
            "INSERT INTO activity (id, media_id, kind, created_at) VALUES
             ('a-1', 'm-1', 'started', '2026-08-03T09:00:00Z'),
             ('a-2', 'm-2', 'completed', '2026-08-03T21:30:00Z'),
             ('a-3', 'm-1', 'progress', '2026-09-01T00:00:00Z'),
             ('a-4', 'm-1', 'progress', '2026-08-03T09:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("seed activity");

        let rows = activity_in_range(&pool, "2026-08-01", "2026-09-01")
            .await
            .expect("activity");
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].title.as_deref(), Some("Book"));
        assert_eq!(rows[0].kind, "started");
        assert_eq!(rows[1].kind, "progress");
        assert_eq!(rows[2].title.as_deref(), Some("Show"));
        assert_eq!(rows[2].kind, "completed");

        sqlx::query("DELETE FROM media WHERE id = 'm-1'")
            .execute(&pool)
            .await
            .expect("delete media");
        let rows = activity_in_range(&pool, "2026-08-01", "2026-09-01")
            .await
            .expect("activity after delete");
        assert_eq!(
            rows.len(),
            1,
            "cascade removes the deleted media's activity"
        );
        assert_eq!(rows[0].title.as_deref(), Some("Show"));
        pool.close().await;
        cleanup_files(&path);
    }
}
