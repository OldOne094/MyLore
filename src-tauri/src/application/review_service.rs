//! Review service (MISSION-074).
//!
//! Single-media user-owned review use-cases: read the review row, save it
//! (validating the domain invariants), clear it, or acknowledge the current
//! content-warning set (MISSION-079). Saving an *empty* review (no rating, no
//! text, no favorite, no mood/pace/content-warning metadata) deletes the row
//! instead of persisting cruft — the persisted row only exists while the user
//! owns at least one bit of review data.
//!
//! Mood/pace/content-warning metadata (MISSION-079) is normalized to a
//! canonical form: keys are validated against the fixed domain vocabulary,
//! deduplicated and sorted. The content-warning acknowledgment timestamp is
//! tied to the *current* warning set — it is preserved on a save that leaves
//! the set unchanged and cleared when the set changes or becomes empty
//! ("acknowledged-with-timestamp metadata, never forced").

use std::collections::BTreeSet;
use std::str::FromStr;

use chrono::Utc;
use sqlx::SqlitePool;

use crate::application::activity_service::log_reviewed;
use crate::domain::review::{ContentWarning, Mood, Pace, Review};
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
    /// Canonical mood keys (sorted, deduplicated).
    pub moods: Vec<String>,
    pub pace: Option<String>,
    /// Canonical content-warning keys (sorted, deduplicated).
    pub content_warnings: Vec<String>,
    /// When the user last acknowledged the current content-warning set.
    pub warnings_acknowledged_at: Option<String>,
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
    pub moods: Vec<String>,
    pub pace: Option<String>,
    pub content_warnings: Vec<String>,
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
    /// (rating bounds, spoiler-requires-text, metadata vocabulary) and
    /// normalizes the metadata. The content-warning acknowledgment is
    /// preserved only when the warning set is unchanged. An entirely empty
    /// review is treated as *clear* — the row is deleted and an empty view is
    /// resolved.
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
        let moods = normalize_moods(&input.moods)?;
        let pace = normalize_pace(input.pace.as_deref())?;
        let content_warnings = normalize_warnings(&input.content_warnings)?;
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
                moods: vec![],
                pace: None,
                content_warnings: vec![],
                warnings_acknowledged_at: None,
                created_at,
                updated_at: now,
            });
        }

        let warnings_acknowledged_at = preserved_acknowledgment(&existing, &content_warnings);

        let domain = Review {
            media_id,
            rating,
            review: input.review.clone(),
            short_review: input.short_review.clone(),
            notes: input.notes.clone(),
            favorite: input.favorite,
            is_spoiler: input.is_spoiler,
            moods,
            pace,
            content_warnings,
            warnings_acknowledged_at,
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
            moods: domain.moods.iter().map(|m| m.as_str().to_string()).collect(),
            pace: domain.pace.map(|p| p.as_str().to_string()),
            content_warnings: domain
                .content_warnings
                .iter()
                .map(|w| w.as_str().to_string())
                .collect(),
            warnings_acknowledged_at: domain.warnings_acknowledged_at,
            created_at: domain.created_at,
            updated_at: domain.updated_at,
        };
        review::upsert(&self.pool, &record).await?;
        log_reviewed(&self.pool, &record.media_id, record.rating).await;

        Ok(view_from_record(&record))
    }

    /// Acknowledge the media's current content-warning set (idempotent — the
    /// stamp is refreshed). Requires an existing review that carries content
    /// warnings; never forced, always the user's explicit action.
    pub async fn acknowledge_warnings(&self, media_id: &str) -> Result<ReviewView, AppError> {
        let media_id = MediaId::new(media_id)?;
        let existing = review::get(&self.pool, media_id.as_str())
            .await?
            .ok_or_else(|| AppError::validation("no review to acknowledge warnings for"))?;
        if existing.content_warnings.is_empty() {
            return Err(AppError::validation(
                "no content warnings to acknowledge",
            ));
        }

        let now = Utc::now().to_rfc3339();
        let record = review::ReviewRecord {
            warnings_acknowledged_at: Some(now.clone()),
            updated_at: now,
            ..existing
        };
        review::upsert(&self.pool, &record).await?;
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

/// An empty review carries no rating, no text, isn't a favorite, and has no
/// mood/pace/content-warning metadata — nothing the user owns, so the row is
/// cleared instead of persisted.
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
        && input.moods.is_empty()
        && input.pace.is_none()
        && input.content_warnings.is_empty()
}

/// Resolve the saved acknowledgment: empty warnings never carry a stamp; an
/// unchanged warning set keeps its existing stamp; any change clears it.
fn preserved_acknowledgment(
    existing: &Option<review::ReviewRecord>,
    content_warnings: &[ContentWarning],
) -> Option<String> {
    if content_warnings.is_empty() {
        return None;
    }
    let new_keys: Vec<String> = content_warnings.iter().map(|w| w.as_str().to_string()).collect();
    match existing {
        Some(row) if row.warnings_acknowledged_at.is_some() && row.content_warnings == new_keys => {
            row.warnings_acknowledged_at.clone()
        }
        _ => None,
    }
}

fn normalize_moods(values: &[String]) -> Result<Vec<Mood>, AppError> {
    let mut moods: BTreeSet<Mood> = BTreeSet::new();
    for value in values {
        moods.insert(Mood::from_str(value.trim()).map_err(AppError::from)?);
    }
    Ok(moods.into_iter().collect())
}

fn normalize_pace(value: Option<&str>) -> Result<Option<Pace>, AppError> {
    match value {
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return Ok(None);
            }
            Pace::from_str(trimmed).map(Some).map_err(AppError::from)
        }
        None => Ok(None),
    }
}

fn normalize_warnings(values: &[String]) -> Result<Vec<ContentWarning>, AppError> {
    let mut warnings: BTreeSet<ContentWarning> = BTreeSet::new();
    for value in values {
        warnings.insert(
            ContentWarning::from_str(value.trim()).map_err(AppError::from)?,
        );
    }
    Ok(warnings.into_iter().collect())
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
        moods: record.moods.clone(),
        pace: record.pace.clone(),
        content_warnings: record.content_warnings.clone(),
        warnings_acknowledged_at: record.warnings_acknowledged_at.clone(),
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
            moods: vec![],
            pace: None,
            content_warnings: vec![],
        }
    }

    fn metadata_input(media_id: &str) -> SaveReviewInput {
        SaveReviewInput {
            media_id: media_id.to_string(),
            rating: None,
            review: None,
            short_review: None,
            notes: None,
            favorite: false,
            is_spoiler: false,
            moods: vec!["tense".to_string(), "dark".to_string()],
            pace: Some("medium".to_string()),
            content_warnings: vec!["violence".to_string(), "gore".to_string()],
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
            moods: vec![],
            pace: None,
            content_warnings: vec![],
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

    #[tokio::test]
    async fn save_persists_normalized_metadata_and_keeps_the_row() {
        let (pool, path) = migrated_pool("review_service_metadata.db").await;
        let service = ReviewService::new(pool.clone());
        seed_media(&pool, "m-1").await;

        // Out-of-order + duplicate input lands canonical (sorted, deduped).
        let mut input = metadata_input("m-1");
        input.moods.push("dark".to_string());
        input.content_warnings.push("violence".to_string());
        let view = service.save(input).await.expect("save metadata");
        assert_eq!(view.moods, vec!["dark", "tense"], "moods canonical");
        assert_eq!(view.pace.as_deref(), Some("medium"));
        assert_eq!(
            view.content_warnings,
            vec!["violence", "gore"],
            "warnings canonical"
        );
        assert!(view.warnings_acknowledged_at.is_none());
        assert!(review::get(&pool, "m-1").await.expect("get").is_some());

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn save_rejects_unknown_mood_pace_and_warning_keys() {
        let (pool, path) = migrated_pool("review_service_badmeta.db").await;
        let service = ReviewService::new(pool.clone());
        seed_media(&pool, "m-1").await;

        let mut bad = metadata_input("m-1");
        bad.moods = vec!["comfy".to_string()];
        assert!(matches!(
            service.save(bad).await.expect_err("unknown mood"),
            AppError::Validation(_)
        ));

        let mut bad = metadata_input("m-1");
        bad.pace = Some("brisk".to_string());
        assert!(matches!(
            service.save(bad).await.expect_err("unknown pace"),
            AppError::Validation(_)
        ));

        let mut bad = metadata_input("m-1");
        bad.content_warnings = vec!["spiders".to_string()];
        assert!(matches!(
            service.save(bad).await.expect_err("unknown warning"),
            AppError::Validation(_)
        ));

        assert!(review::get(&pool, "m-1").await.expect("get").is_none());

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn acknowledgment_is_preserved_only_for_the_unchanged_set() {
        let (pool, path) = migrated_pool("review_service_ack.db").await;
        let service = ReviewService::new(pool.clone());
        seed_media(&pool, "m-1").await;

        service.save(metadata_input("m-1")).await.expect("save");
        service.acknowledge_warnings("m-1").await.expect("acknowledge");
        let acknowledged = review::get(&pool, "m-1").await.expect("get").unwrap();
        assert!(acknowledged.warnings_acknowledged_at.is_some());
        let stamp = acknowledged.warnings_acknowledged_at.clone();

        // Same set → stamp preserved across a save.
        let view = service.save(metadata_input("m-1")).await.expect("re-save");
        assert_eq!(view.warnings_acknowledged_at, stamp);

        // Changed set → stamp cleared (the new set is unacknowledged).
        let mut changed = metadata_input("m-1");
        changed.content_warnings = vec!["death".to_string()];
        let view = service.save(changed).await.expect("changed warnings");
        assert!(view.warnings_acknowledged_at.is_none());

        // Acknowledging again re-stamps the new set.
        let view = service
            .acknowledge_warnings("m-1")
            .await
            .expect("re-acknowledge");
        assert!(view.warnings_acknowledged_at.is_some());

        // Empty set → stamp cleared.
        let mut cleared = metadata_input("m-1");
        cleared.content_warnings = vec![];
        let view = service.save(cleared).await.expect("cleared warnings");
        assert!(view.warnings_acknowledged_at.is_none());

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn acknowledge_warnings_requires_a_review_and_a_warning_set() {
        let (pool, path) = migrated_pool("review_service_ack_req.db").await;
        let service = ReviewService::new(pool.clone());
        seed_media(&pool, "m-1").await;

        assert!(matches!(
            service
                .acknowledge_warnings("m-1")
                .await
                .expect_err("no review"),
            AppError::Validation(_)
        ));

        service.save(input("m-1")).await.expect("save plain review");
        assert!(matches!(
            service
                .acknowledge_warnings("m-1")
                .await
                .expect_err("no warnings"),
            AppError::Validation(_)
        ));

        pool.close().await;
        cleanup_files(&path);
    }
}
