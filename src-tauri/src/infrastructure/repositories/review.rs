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
    pub created_at: String,
    pub updated_at: String,
}

/// Insert or update the review for a media, preserving the original created_at.
pub async fn upsert(pool: &SqlitePool, r: &ReviewRecord) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO review
           (media_id, rating, review, short_review, notes, favorite, is_spoiler, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(media_id) DO UPDATE SET
           rating = excluded.rating,
           review = excluded.review,
           short_review = excluded.short_review,
           notes = excluded.notes,
           favorite = excluded.favorite,
           is_spoiler = excluded.is_spoiler,
           updated_at = excluded.updated_at",
    )
    .bind(&r.media_id)
    .bind(r.rating)
    .bind(&r.review)
    .bind(&r.short_review)
    .bind(&r.notes)
    .bind(r.favorite)
    .bind(r.is_spoiler)
    .bind(&r.created_at)
    .bind(&r.updated_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Fetch a media's review (or `None`).
pub async fn get(pool: &SqlitePool, media_id: &str) -> Result<Option<ReviewRecord>, AppError> {
    let row = sqlx::query(
        "SELECT media_id, rating, review, short_review, notes, favorite, is_spoiler, \
         created_at, updated_at FROM review WHERE media_id = ?",
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
        created_at: get(7).expect("created_at"),
        updated_at: get(8).expect("updated_at"),
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
}
