//! Review service (MISSION-074).
//!
//! Single-media user-owned review use-cases: read the review row, save it
//! (validating the domain invariants), or clear it. Saving an *empty* review
//! (no rating, no text, not a favorite) deletes the row instead of persisting
//! cruft — the persisted row only exists while the user owns at least one bit
//! of review data.

use chrono::Utc;
use sqlx::SqlitePool;

use crate::application::activity_service::log_reviewed;
use crate::domain::review::Review;
use crate::domain::value_objects::{MediaId, Rating};
use crate::error::AppError;
use crate::infrastructure::repositories::{media, review};

/// The review row surfaced to the detail page's Review tab.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ReviewView {
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

/// Command input for saving a review (mirrors the IPC contract).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SaveReviewInput {
    pub media_id: String,
    pub rating: Option<i64>,
    pub review: Option<String>,
    pub short_review: Option<String>,
    pub notes: Option<String>,
    pub favorite: bool,
    pub is_spoiler: bool,
}

/// Review use-cases for a single media.
pub struct ReviewService {
    pool: SqlitePool,
}

impl ReviewService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Read a media's review (`None` when the user hasn't reviewed it).
    pub async fn get(&self, media_id: &str) -> Result<Option<ReviewView>, AppError> {
        let record = review::get(&self.pool, media_id).await?;
        Ok(record.map(|row| view_from_record(&row)))
    }

    /// Save (create or update) a media's review. Preserves the original
    /// `created_at`, stamps `updated_at` now, validates the domain invariants
    /// (rating bounds, spoiler-requires-text). An entirely empty review is
    /// treated as *clear* — the row is deleted and an empty view is resolved.
    pub async fn save(&self, input: SaveReviewInput) -> Result<ReviewView, AppError> {
        let media_id = MediaId::new(&input.media_id)?;
        if media::get(&self.pool, media_id.as_str()).await?.is_none() {
            return Err(AppError::validation(format!(
                "media not found: {}",
                media_id.as_str()
            )));
        }

        let now = Utc::now().to_rfc3339();
        let rating = input.rating.map(i64_to_rating).transpose()?;
        let existing = review::get(&self.pool, media_id.as_str()).await?;
        let created_at = existing
            .as_ref()
            .map(|row| row.created_at.clone())
            .unwrap_or_else(|| now.clone());

        if is_empty(&input) {
            review::delete(&self.pool, media_id.as_str()).await?;
            return Ok(ReviewView {
                media_id: media_id.as_str().to_string(),
                rating: None,
                review: None,
                short_review: None,
                notes: None,
                favorite: false,
                is_spoiler: false,
                created_at,
                updated_at: now,
            });
        }

        let domain = Review {
            media_id,
            rating,
            review: input.review.clone(),
            short_review: input.short_review.clone(),
            notes: input.notes.clone(),
            favorite: input.favorite,
            is_spoiler: input.is_spoiler,
            created_at: created_at.clone(),
            updated_at: now.clone(),
        };
        domain.validate()?;

        let record = review::ReviewRecord {
            media_id: domain.media_id.as_str().to_string(),
            rating: domain.rating.map(|r| i64::from(r.get())),
            review: domain.review,
            short_review: domain.short_review,
            notes: domain.notes,
            favorite: domain.favorite,
            is_spoiler: domain.is_spoiler,
            created_at: domain.created_at,
            updated_at: domain.updated_at,
        };
        review::upsert(&self.pool, &record).await?;
        log_reviewed(&self.pool, &record.media_id, record.rating).await;

        Ok(view_from_record(&record))
    }

    /// Delete a media's review row (idempotent — `None` for an unknown media is
    /// an error so the UI can't silently clear against a typo'd id).
    pub async fn delete(&self, media_id: &str) -> Result<(), AppError> {
        let media_id = MediaId::new(media_id)?;
        if media::get(&self.pool, media_id.as_str()).await?.is_none() {
            return Err(AppError::validation(format!(
                "media not found: {}",
                media_id.as_str()
            )));
        }
        review::delete(&self.pool, media_id.as_str()).await?;
        Ok(())
    }
}

/// An empty review carries no rating, no text, and isn't a favorite — nothing
/// the user owns, so the row is cleared instead of persisted.
fn is_empty(input: &SaveReviewInput) -> bool {
    input.rating.is_none()
        && input
            .review
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .is_empty()
        && input
            .short_review
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .is_empty()
        && input
            .notes
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .is_empty()
        && !input.favorite
}

fn i64_to_rating(value: i64) -> Result<Rating, AppError> {
    let value = u8::try_from(value)
        .map_err(|_| AppError::validation(format!("rating out of range: {value}")))?;
    Rating::new(i64::from(value))
        .map_err(|err| AppError::validation(format!("rating out of range: {err}")))
}

fn view_from_record(record: &review::ReviewRecord) -> ReviewView {
    ReviewView {
        media_id: record.media_id.clone(),
        rating: record.rating,
        review: record.review.clone(),
        short_review: record.short_review.clone(),
        notes: record.notes.clone(),
        favorite: record.favorite,
        is_spoiler: record.is_spoiler,
        created_at: record.created_at.clone(),
        updated_at: record.updated_at.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::test_support::{cleanup_files, migrated_pool};

    async fn seed_media(pool: &sqlx::SqlitePool, id: &str) {
        sqlx::query(
            "INSERT INTO media (id, content_type, title_main, created_at, updated_at)
             VALUES (?, 'novel', 'Seed Title', '2026-01-01', '2026-01-01')",
        )
        .bind(id)
        .execute(pool)
        .await
        .expect("seed media");
    }

    fn input(media_id: &str) -> SaveReviewInput {
        SaveReviewInput {
            media_id: media_id.to_string(),
            rating: Some(8),
            review: Some("A sweeping epic".to_string()),
            short_review: None,
            notes: None,
            favorite: true,
            is_spoiler: false,
        }
    }

    #[tokio::test]
    async fn save_creates_then_updates_preserving_created_at() {
        let (pool, path) = migrated_pool("review_service_save.db").await;
        let service = ReviewService::new(pool.clone());
        seed_media(&pool, "m-1").await;

        let view = service.save(input("m-1")).await.expect("save");
        assert_eq!(view.rating, Some(8));
        assert_eq!(view.review.as_deref(), Some("A sweeping epic"));
        assert!(view.favorite);
        assert!(!view.is_spoiler);
        let first_created = view.created_at.clone();

        let mut updated = input("m-1");
        updated.rating = Some(9);
        updated.review = None;
        updated.favorite = false;
        let view = service.save(updated).await.expect("update");
        assert_eq!(view.rating, Some(9));
        assert_eq!(view.review, None);
        assert!(!view.favorite);
        assert_eq!(view.created_at, first_created, "created_at preserved");

        let stored = review::get(&pool, "m-1").await.expect("get").unwrap();
        assert_eq!(stored.created_at, first_created);
        assert_eq!(stored.updated_at, view.updated_at);
        assert!(stored.updated_at >= stored.created_at);

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn save_rejects_rating_out_of_range() {
        let (pool, path) = migrated_pool("review_service_rating.db").await;
        let service = ReviewService::new(pool.clone());
        seed_media(&pool, "m-1").await;

        let mut bad = input("m-1");
        bad.rating = Some(11);
        let err = service.save(bad).await.expect_err("out-of-range rating");
        assert!(matches!(err, AppError::Validation(_)));

        let mut bad = input("m-1");
        bad.rating = Some(0);
        assert!(matches!(
            service.save(bad).await.expect_err("zero rating"),
            AppError::Validation(_)
        ));

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn save_rejects_spoiler_without_text() {
        let (pool, path) = migrated_pool("review_service_spoiler.db").await;
        let service = ReviewService::new(pool.clone());
        seed_media(&pool, "m-1").await;

        let mut bad = input("m-1");
        bad.review = None;
        bad.short_review = None;
        bad.notes = None;
        bad.is_spoiler = true;
        let err = service.save(bad).await.expect_err("spoiler w/o text");
        assert!(matches!(err, AppError::Validation(_)));
        assert!(review::get(&pool, "m-1").await.expect("get").is_none());

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn save_empty_review_deletes_the_row() {
        let (pool, path) = migrated_pool("review_service_empty.db").await;
        let service = ReviewService::new(pool.clone());
        seed_media(&pool, "m-1").await;

        service.save(input("m-1")).await.expect("save");
        assert!(review::get(&pool, "m-1").await.expect("get").is_some());

        let empty = SaveReviewInput {
            media_id: "m-1".to_string(),
            rating: None,
            review: Some("   ".to_string()),
            short_review: None,
            notes: None,
            favorite: false,
            is_spoiler: false,
        };
        let view = service.save(empty).await.expect("clear");
        assert_eq!(view.rating, None);
        assert!(review::get(&pool, "m-1").await.expect("get").is_none());

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn get_returns_none_until_a_review_exists() {
        let (pool, path) = migrated_pool("review_service_get.db").await;
        let service = ReviewService::new(pool.clone());
        seed_media(&pool, "m-1").await;

        assert!(service.get("m-1").await.expect("get").is_none());
        service.save(input("m-1")).await.expect("save");
        let view = service.get("m-1").await.expect("get").unwrap();
        assert_eq!(view.media_id, "m-1");
        assert_eq!(view.rating, Some(8));

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn delete_removes_the_row_and_is_repeatable() {
        let (pool, path) = migrated_pool("review_service_delete.db").await;
        let service = ReviewService::new(pool.clone());
        seed_media(&pool, "m-1").await;

        service.save(input("m-1")).await.expect("save");
        service.delete("m-1").await.expect("delete");
        assert!(review::get(&pool, "m-1").await.expect("get").is_none());
        service.delete("m-1").await.expect("delete again");

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn save_and_delete_reject_unknown_media() {
        let (pool, path) = migrated_pool("review_service_unknown.db").await;
        let service = ReviewService::new(pool.clone());

        assert!(matches!(
            service.save(input("m-ghost")).await.expect_err("unknown"),
            AppError::Validation(_)
        ));
        assert!(matches!(
            service.delete("m-ghost").await.expect_err("unknown"),
            AppError::Validation(_)
        ));

        pool.close().await;
        cleanup_files(&path);
    }
}
