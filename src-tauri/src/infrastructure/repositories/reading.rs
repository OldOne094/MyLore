//! Reading recap repository (MISSION-083).
//!
//! Three small queries feed the reading recap: consumed nodes within a
//! timestamp window (pages/chapters per month), the review taste rows (moods +
//! pace across every review), and the format distribution of tracked reading
//! media. The monthly query returns raw rows — month bucketing happens in the
//! service so it can use the user's local timezone.

use sqlx::sqlite::{SqlitePool, SqliteRow};
use sqlx::Row;

use crate::error::AppError;

/// Reading content types (everything except anime/tv/movie/other).
pub const READING_TYPES: &str = "'book','novel','web_novel','manga','manhwa','manhua'";

/// One consumed node within the window.
#[derive(Debug, Clone)]
pub struct MonthlyReadingRow {
    /// RFC3339 completion timestamp (node_progress.read_at).
    pub read_at: String,
    pub page_count: Option<i64>,
    pub content_type: String,
}

/// Consumed nodes within `[from, to)` for reading media (RFC3339 lexicographic).
pub async fn monthly_reading(
    pool: &SqlitePool,
    from: &str,
    to: &str,
) -> Result<Vec<MonthlyReadingRow>, AppError> {
    let rows = sqlx::query(
        format!(
            "SELECT np.read_at, cn.page_count, m.content_type \
             FROM node_progress np \
             JOIN content_node cn ON cn.id = np.node_id \
             JOIN media m ON m.id = cn.media_id \
             WHERE np.state IN ('read', 'watched') \
               AND np.read_at IS NOT NULL \
               AND m.content_type IN ({READING_TYPES}) \
               AND np.read_at >= ? AND np.read_at < ? \
             ORDER BY np.read_at, np.node_id"
        )
        .as_str(),
    )
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(row_to_reading).collect())
}

/// A review's taste metadata for the recap distribution charts.
#[derive(Debug, Clone)]
pub struct TasteRow {
    /// Stored JSON array of mood keys (`[]` for NULL).
    pub moods: Option<String>,
    pub pace: Option<String>,
}

/// Every review's mood set + pace (one row per reviewed media).
pub async fn taste_rows(pool: &SqlitePool) -> Result<Vec<TasteRow>, AppError> {
    let rows = sqlx::query("SELECT moods, pace FROM review")
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| TasteRow {
            moods: row.get(0),
            pace: row.get(1),
        })
        .collect())
}

/// A format ranked by how many tracked reading media carry it.
#[derive(Debug, Clone, PartialEq)]
pub struct FormatCountRow {
    pub format: String,
    pub count: u32,
}

/// Format distribution among tracked reading media (formats with no tracked
/// reading media are absent).
pub async fn reading_formats(pool: &SqlitePool) -> Result<Vec<FormatCountRow>, AppError> {
    let rows = sqlx::query(
        format!(
            "SELECT m.format, COUNT(*) AS cnt \
             FROM tracking t JOIN media m ON m.id = t.media_id \
             WHERE m.format IS NOT NULL AND m.content_type IN ({READING_TYPES}) \
             GROUP BY m.format ORDER BY cnt DESC, m.format"
        )
        .as_str(),
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| FormatCountRow {
            format: row.get(0),
            count: row.get::<i64, _>(1) as u32,
        })
        .collect())
}

fn row_to_reading(row: SqliteRow) -> MonthlyReadingRow {
    MonthlyReadingRow {
        read_at: row.get("read_at"),
        page_count: row.get("page_count"),
        content_type: row.get("content_type"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::test_support::{cleanup_files, migrated_pool};

    async fn seed_media(pool: &SqlitePool, id: &str, content_type: &str, format: Option<&str>) {
        sqlx::query(
            "INSERT INTO media (id, content_type, format, title_main, created_at, updated_at)
             VALUES (?, ?, ?, 'Title', '2026-01-01', '2026-01-01')",
        )
        .bind(id)
        .bind(content_type)
        .bind(format)
        .execute(pool)
        .await
        .expect("seed media");
    }

    async fn seed_node(pool: &SqlitePool, id: &str, media_id: &str, page_count: Option<i64>) {
        sqlx::query(
            "INSERT INTO content_node (id, media_id, kind, position, number, page_count, created_at)
             VALUES (?, ?, 'chapter', 1, ?, ?, '2026-01-01')",
        )
        .bind(id)
        .bind(media_id)
        .bind(id)
        .bind(page_count)
        .execute(pool)
        .await
        .expect("seed node");
    }

    async fn seed_progress(pool: &SqlitePool, node_id: &str, read_at: &str) {
        sqlx::query(
            "INSERT INTO node_progress (node_id, state, read_at, updated_at)
             VALUES (?, 'read', ?, ?)",
        )
        .bind(node_id)
        .bind(read_at)
        .bind(read_at)
        .execute(pool)
        .await
        .expect("seed progress");
    }

    async fn track(pool: &SqlitePool, media_id: &str) {
        sqlx::query(
            "INSERT INTO tracking (media_id, core_status, auto_track, updated_at)
             VALUES (?, 'in_progress', 1, '2026-01-01')",
        )
        .bind(media_id)
        .execute(pool)
        .await
        .expect("track");
    }

    #[tokio::test]
    async fn monthly_reading_returns_consumed_nodes_in_window() {
        let (pool, path) = migrated_pool("reading_monthly.db").await;
        seed_media(&pool, "m-1", "book", Some("light_novel")).await;
        seed_media(&pool, "m-2", "manga", None).await;
        seed_media(&pool, "m-3", "anime", None).await;
        seed_node(&pool, "n-1", "m-1", Some(120)).await;
        seed_node(&pool, "n-2", "m-2", None).await;
        seed_node(&pool, "n-3", "m-3", None).await;
        seed_progress(&pool, "n-1", "2026-08-03T09:00:00Z").await;
        seed_progress(&pool, "n-2", "2026-08-05T12:00:00Z").await;
        seed_progress(&pool, "n-3", "2026-08-05T12:00:00Z").await;

        let rows = monthly_reading(&pool, "2026-08-01", "2026-09-01")
            .await
            .expect("monthly reading");
        assert_eq!(rows.len(), 2, "the anime episode is excluded");
        assert!(rows.iter().all(|r| r.content_type != "anime"));
        assert!(rows.iter().any(|r| r.page_count == Some(120)));

        let none = monthly_reading(&pool, "2026-09-01", "2026-10-01")
            .await
            .expect("empty window");
        assert!(none.is_empty());

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn reading_formats_counts_tracked_reading_media() {
        let (pool, path) = migrated_pool("reading_formats.db").await;
        seed_media(&pool, "m-1", "novel", Some("light_novel")).await;
        seed_media(&pool, "m-2", "novel", Some("light_novel")).await;
        seed_media(&pool, "m-3", "manga", Some("webtoon")).await;
        seed_media(&pool, "m-4", "manga", None).await;
        seed_media(&pool, "m-5", "anime", Some("tv")).await;
        for id in ["m-1", "m-2", "m-3", "m-4", "m-5"] {
            track(&pool, id).await;
        }

        let rows = reading_formats(&pool).await.expect("formats");
        assert_eq!(
            rows,
            vec![
                FormatCountRow {
                    format: "light_novel".into(),
                    count: 2
                },
                FormatCountRow {
                    format: "webtoon".into(),
                    count: 1
                },
            ],
            "the anime format and the format-less manga are excluded"
        );

        pool.close().await;
        cleanup_files(&path);
    }
}
