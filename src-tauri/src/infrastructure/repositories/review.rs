//! Review repository (MISSION-019).

use sqlx::sqlite::{SqlitePool, SqliteRow};
use sqlx::Row;

use crate::error::AppError;

/// A review (one per media).
#[derive(Debug, Clone)]
pub struct ReviewRecord {
    pub media_id: String,
    pub rating: Option<i64>,
    pub review: Option<String>,
    pub short_review: Option<String>,
    pub notes: Option<String>,
    pub favorite: bool,
    pub is_spoiler: bool,
    /// Canonical mood keys (sorted, deduplicated — MISSION-079).
    pub moods: Vec<String>,
    pub pace: Option<String>,
    /// Canonical content-warning keys (sorted, deduplicated — MISSION-079).
    pub content_warnings: Vec<String>,
    pub warnings_acknowledged_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Insert or update the review for a media, preserving the original created_at.
pub async fn upsert(pool: &SqlitePool, r: &ReviewRecord) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO review
           (media_id, rating, review, short_review, notes, favorite, is_spoiler, moods, pace,
            content_warnings, warnings_acknowledged_at, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(media_id) DO UPDATE SET
           rating = excluded.rating,
           review = excluded.review,
           short_review = excluded.short_review,
           notes = excluded.notes,
           favorite = excluded.favorite,
           is_spoiler = excluded.is_spoiler,
           moods = excluded.moods,
           pace = excluded.pace,
           content_warnings = excluded.content_warnings,
           warnings_acknowledged_at = excluded.warnings_acknowledged_at,
           updated_at = excluded.updated_at",
    )
    .bind(&r.media_id)
    .bind(r.rating)
    .bind(&r.review)
    .bind(&r.short_review)
    .bind(&r.notes)
    .bind(r.favorite)
    .bind(r.is_spoiler)
    .bind(json_string(&r.moods))
    .bind(&r.pace)
    .bind(json_string(&r.content_warnings))
    .bind(&r.warnings_acknowledged_at)
    .bind(&r.created_at)
    .bind(&r.updated_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// [`upsert`] inside the caller's transaction (used by the import pipeline,
/// MISSION-072).
pub async fn upsert_in_tx<'e>(
    tx: &mut sqlx::Transaction<'e, sqlx::Sqlite>,
    r: &ReviewRecord,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO review
           (media_id, rating, review, short_review, notes, favorite, is_spoiler, moods, pace,
            content_warnings, warnings_acknowledged_at, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(media_id) DO UPDATE SET
           rating = excluded.rating,
           review = excluded.review,
           short_review = excluded.short_review,
           notes = excluded.notes,
           favorite = excluded.favorite,
           is_spoiler = excluded.is_spoiler,
           moods = excluded.moods,
           pace = excluded.pace,
           content_warnings = excluded.content_warnings,
           warnings_acknowledged_at = excluded.warnings_acknowledged_at,
           updated_at = excluded.updated_at",
    )
    .bind(&r.media_id)
    .bind(r.rating)
    .bind(&r.review)
    .bind(&r.short_review)
    .bind(&r.notes)
    .bind(r.favorite)
    .bind(r.is_spoiler)
    .bind(json_string(&r.moods))
    .bind(&r.pace)
    .bind(json_string(&r.content_warnings))
    .bind(&r.warnings_acknowledged_at)
    .bind(&r.created_at)
    .bind(&r.updated_at)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Fetch a media's review (or `None`).
pub async fn get(pool: &SqlitePool, media_id: &str) -> Result<Option<ReviewRecord>, AppError> {
    let row = sqlx::query(
        "SELECT media_id, rating, review, short_review, notes, favorite, is_spoiler, moods, pace, \
         content_warnings, warnings_acknowledged_at, created_at, updated_at \
         FROM review WHERE media_id = ?",
    )
    .bind(media_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(row_to_review))
}

/// Delete a media's review.
pub async fn delete(pool: &SqlitePool, media_id: &str) -> Result<(), AppError> {
    sqlx::query("DELETE FROM review WHERE media_id = ?")
        .bind(media_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Serialize a list of keys for storage as a JSON array column.
fn json_string(keys: &[String]) -> String {
    serde_json::to_string(keys).expect("serialize key list")
}

/// Parse a stored JSON array column back into keys (`[]` for NULL / garbage —
/// the column is only ever written by this repo with canonical JSON).
fn parse_keys(value: Option<String>) -> Vec<String> {
    match value {
        Some(raw) => serde_json::from_str::<Vec<String>>(&raw).unwrap_or_default(),
        None => Vec::new(),
    }
}

fn row_to_review(row: SqliteRow) -> ReviewRecord {
    let get = |idx: usize| -> Option<String> { row.get(idx) };
    ReviewRecord {
        media_id: get(0).expect("media_id"),
        rating: row.get(1),
        review: get(2),
        short_review: get(3),
        notes: get(4),
        favorite: row.get::<i64, _>(5) != 0,
        is_spoiler: row.get::<i64, _>(6) != 0,
        moods: parse_keys(get(7)),
        pace: get(8),
        content_warnings: parse_keys(get(9)),
        warnings_acknowledged_at: get(10),
        created_at: get(11).expect("created_at"),
        updated_at: get(12).expect("updated_at"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::test_support::{cleanup_files, migrated_pool};

    fn review(media_id: &str) -> ReviewRecord {
        ReviewRecord {
            media_id: media_id.to_string(),
            rating: Some(8),
            review: Some("Great".into()),
            short_review: None,
            notes: None,
            favorite: true,
            is_spoiler: false,
            moods: vec!["dark".to_string(), "tense".to_string()],
            pace: Some("medium".to_string()),
            content_warnings: vec!["violence".to_string()],
            warnings_acknowledged_at: Some("2026-01-02T00:00:00Z".to_string()),
            created_at: "2026-01-01".to_string(),
            updated_at: "2026-01-01".to_string(),
        }
    }

    #[tokio::test]
    async fn upsert_preserves_created_at_and_deletes() {
        let (pool, path) = migrated_pool("review_repo.db").await;
        sqlx::query(
            "INSERT INTO media (id, content_type, title_main, created_at, updated_at)
             VALUES ('m-1', 'novel', 'Title', '2026-01-01', '2026-01-01')",
        )
        .execute(&pool)
        .await
        .expect("seed media");

        upsert(&pool, &review("m-1")).await.expect("upsert");
        let mut r = review("m-1");
        r.rating = Some(9);
        r.favorite = false;
        r.updated_at = "2026-02-01".into();
        upsert(&pool, &r).await.expect("re-upsert");

        let got = get(&pool, "m-1").await.expect("get").unwrap();
        assert_eq!(got.rating, Some(9));
        assert!(!got.favorite);
        assert_eq!(got.moods, vec!["dark", "tense"], "metadata persists");
        assert_eq!(got.pace.as_deref(), Some("medium"));
        assert_eq!(got.content_warnings, vec!["violence"]);
        assert_eq!(
            got.warnings_acknowledged_at.as_deref(),
            Some("2026-01-02T00:00:00Z")
        );
        assert_eq!(
            got.created_at, "2026-01-01",
            "original created_at preserved"
        );
        assert_eq!(got.updated_at, "2026-02-01");

        delete(&pool, "m-1").await.expect("delete");
        assert!(get(&pool, "m-1").await.expect("get").is_none());
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn null_metadata_reads_back_as_empty() {
        let (pool, path) = migrated_pool("review_repo_null.db").await;
        sqlx::query(
            "INSERT INTO media (id, content_type, title_main, created_at, updated_at)
             VALUES ('m-1', 'novel', 'Title', '2026-01-01', '2026-01-01')",
        )
        .execute(&pool)
        .await
        .expect("seed media");

        let mut r = review("m-1");
        r.moods = vec![];
        r.pace = None;
        r.content_warnings = vec![];
        r.warnings_acknowledged_at = None;
        upsert(&pool, &r).await.expect("upsert");

        let got = get(&pool, "m-1").await.expect("get").unwrap();
        assert!(got.moods.is_empty());
        assert!(got.pace.is_none());
        assert!(got.content_warnings.is_empty());
        assert!(got.warnings_acknowledged_at.is_none());
        pool.close().await;
        cleanup_files(&path);
    }
}
