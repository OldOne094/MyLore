//! Bulk operations application service (MISSION-045).
//!
//! Actions behind the library action bar: set tracking status, add a personal
//! tag, and soft-delete (to trash). MISSION-078 extends this so an operation
//! can target either an explicit id list **or** the whole filtered selection
//! (resolved server-side from a facet filter), and every operation resolves
//! with a per-item change summary instead of failing the whole batch on the
//! first bad media. (Add-to-collection lives in `collection_service` since
//! MISSION-076.)
//!
//! Status uses the domain status engine per media, so stamps (started_at /
//! finished_at / repeat_count) stay correct and the Repeat-guard still applies.
//! A media that cannot move to the target is reported in the summary's
//! `failures` and the rest of the batch still applies.

use std::str::FromStr;

use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::application::activity_service::log_status_transition;
use crate::application::media_service::normalize_tag;
use crate::application::tracking_service::{domain_to_record, existing_to_domain};
use crate::application::trash_service::TrashService;
use crate::domain::enums::CoreStatus;
use crate::domain::status::apply_transition;
use crate::domain::value_objects::{DateOnly, MediaId};
use crate::error::AppError;
use crate::infrastructure::repositories::media::MediaFilter;
use crate::infrastructure::repositories::{media, tracking};

/// Optional facet filter describing the media set to operate on (MISSION-078).
/// When present, the target media are resolved server-side from this filter
/// instead of the caller's explicit `ids` — bulk "apply to all N matching".
#[derive(Debug, Clone, serde::Deserialize)]
pub struct BulkFilter {
    pub content_type: Option<String>,
    pub format: Option<String>,
    pub pub_status: Option<String>,
    pub genre: Option<String>,
    pub tag: Option<String>,
    pub year: Option<i64>,
    pub favorite: Option<bool>,
}

/// One media the operation could not process.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BulkFailure {
    pub media_id: String,
    pub reason: String,
}

/// Per-item outcome of a bulk operation (MISSION-078). A non-zero `failed`
/// count means some media were skipped; their ids and reasons are in
/// `failures` so the UI can surface them.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BulkResult {
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub failures: Vec<BulkFailure>,
}

/// Soft-delete outcome: the summary plus a trash id per successfully deleted
/// media, so a group undo restores exactly what was removed.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BulkDeleteResult {
    pub summary: BulkResult,
    pub trash_ids: Vec<String>,
}

/// Resolve the media ids a bulk operation should touch. When a filter is given
/// the set is computed server-side against the same query the library uses;
/// otherwise the caller's explicit ids are used as-is.
pub async fn resolve_targets(
    pool: &SqlitePool,
    filter: Option<&BulkFilter>,
    ids: &[String],
) -> Result<Vec<String>, AppError> {
    let Some(filter) = filter else {
        return Ok(ids.to_vec());
    };
    let filter = MediaFilter {
        content_type: filter.content_type.clone(),
        format: filter.format.clone(),
        pub_status: filter.pub_status.clone(),
        genre: filter.genre.clone(),
        tag: filter.tag.clone(),
        year: filter.year,
        favorite: filter.favorite,
        search: None,
        sort: Default::default(),
        ascending: true,
        limit: None,
        offset: None,
    };
    let rows = media::list(pool, &filter).await?;
    Ok(rows.into_iter().map(|row| row.id).collect())
}

/// Bulk use-cases over a set of media ids (or a filtered selection).
pub struct BulkService {
    pool: SqlitePool,
}

impl BulkService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Set the tracking status for many media at once. Media without a tracking
    /// row start from `Planned`; every media moves through the status engine so
    /// side-effect stamps stay correct. Media that cannot move to the target
    /// (e.g. Repeat without consumption) are reported in the summary instead of
    /// aborting the batch; their tracking rows are left untouched.
    pub async fn set_status(
        &self,
        filter: Option<BulkFilter>,
        ids: &[String],
        core_status: &str,
    ) -> Result<BulkResult, AppError> {
        let to = CoreStatus::from_str(core_status)?;
        let today = DateOnly::new(Utc::now().format("%Y-%m-%d").to_string())?;
        let updated_at = Utc::now().to_rfc3339();
        let targets = resolve_targets(&self.pool, filter.as_ref(), ids).await?;
        let mut result = BulkResult {
            total: targets.len(),
            succeeded: 0,
            failed: 0,
            failures: Vec::new(),
        };

        for raw_id in &targets {
            let outcome = async {
                let media_id = MediaId::new(raw_id)?;
                let existing = tracking::get_tracking(&self.pool, media_id.as_str()).await?;
                let current = existing_to_domain(existing, &media_id, &updated_at)?;
                let next = apply_transition(&current, to, &today)?;
                let mut next = next;
                next.updated_at = updated_at.clone();
                tracking::upsert_tracking(&self.pool, &domain_to_record(&next)).await?;
                log_status_transition(
                    &self.pool,
                    media_id.as_str(),
                    &current.core_status,
                    &next.core_status,
                )
                .await;
                Ok::<_, AppError>(())
            };
            match outcome.await {
                Ok(()) => result.succeeded += 1,
                Err(err) => {
                    result.failed += 1;
                    result.failures.push(BulkFailure {
                        media_id: raw_id.clone(),
                        reason: err.to_string(),
                    });
                }
            }
        }
        Ok(result)
    }

    /// Add a personal tag to many media. The tag row is reused when one with the
    /// same name already exists, else created (scope `personal`); media already
    /// carrying the tag are skipped. Media that cannot be tagged (unknown id)
    /// land in the summary's failures.
    pub async fn add_tag(
        &self,
        filter: Option<BulkFilter>,
        ids: &[String],
        tag: &str,
    ) -> Result<BulkResult, AppError> {
        let name = normalize_tag(tag)?;
        let tag_id = match media::resolve_personal_tag(&self.pool, &name).await? {
            Some(id) => id,
            None => {
                let id = format!("tag-{}", Uuid::new_v4());
                media::create_personal_tag(&self.pool, &id, &name).await?;
                id
            }
        };
        let targets = resolve_targets(&self.pool, filter.as_ref(), ids).await?;
        let mut result = BulkResult {
            total: targets.len(),
            succeeded: 0,
            failed: 0,
            failures: Vec::new(),
        };

        for raw_id in &targets {
            match media::add_tag_to_many(&self.pool, &tag_id, std::slice::from_ref(raw_id)).await {
                Ok(_) => result.succeeded += 1,
                Err(err) => {
                    result.failed += 1;
                    result.failures.push(BulkFailure {
                        media_id: raw_id.clone(),
                        reason: err.to_string(),
                    });
                }
            }
        }
        Ok(result)
    }

    /// Soft-delete many media. Resolves with one trash id per successfully
    /// deleted media (for a single "undo" over exactly what was removed);
    /// unknown media land in the summary's failures.
    pub async fn delete(
        &self,
        filter: Option<BulkFilter>,
        ids: &[String],
    ) -> Result<BulkDeleteResult, AppError> {
        let targets = resolve_targets(&self.pool, filter.as_ref(), ids).await?;
        let trash_service = TrashService::new(self.pool.clone());
        let mut summary = BulkResult {
            total: targets.len(),
            succeeded: 0,
            failed: 0,
            failures: Vec::new(),
        };
        let mut trash_ids = Vec::new();
        for raw_id in &targets {
            match trash_service.delete_media(raw_id).await {
                Ok(trash_id) => {
                    summary.succeeded += 1;
                    trash_ids.push(trash_id);
                }
                Err(err) => {
                    summary.failed += 1;
                    summary.failures.push(BulkFailure {
                        media_id: raw_id.clone(),
                        reason: err.to_string(),
                    });
                }
            }
        }
        Ok(BulkDeleteResult { summary, trash_ids })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::media_service::{AddMediaInput, MediaListInput, MediaService};
    use crate::infrastructure::repositories::{activity, tracking};
    use crate::infrastructure::test_support::migrated_pool;

    fn input(title: &str) -> AddMediaInput {
        AddMediaInput {
            title: title.into(),
            content_type: "novel".into(),
            format: None,
            pub_status: None,
            synopsis: None,
            release_year: None,
            language: None,
            country: None,
            pages: None,
            duration_min: None,
            ep_count: None,
            ch_count: None,
            genres: vec![],
        }
    }

    fn input_type(title: &str, content_type: &str) -> AddMediaInput {
        let mut input = input(title);
        input.content_type = content_type.into();
        input
    }

    fn filter(content_type: &str) -> BulkFilter {
        BulkFilter {
            content_type: Some(content_type.into()),
            format: None,
            pub_status: None,
            genre: None,
            tag: None,
            year: None,
            favorite: None,
        }
    }

    async fn seed_media(pool: &SqlitePool, count: u32) -> Vec<String> {
        let media_service = MediaService::new(pool.clone());
        let mut ids = Vec::new();
        for index in 0..count {
            ids.push(
                media_service
                    .add_media(input(&format!("Title {index}")))
                    .await
                    .expect("add media")
                    .as_str()
                    .to_string(),
            );
        }
        ids
    }

    #[tokio::test]
    async fn set_status_creates_tracking_and_stamps_start() {
        let (pool, path) = migrated_pool("bulk_status_create.db").await;
        let ids = seed_media(&pool, 2).await;
        let service = BulkService::new(pool.clone());

        let result = service
            .set_status(None, &ids, "in_progress")
            .await
            .expect("set status");
        assert_eq!(result.total, 2);
        assert_eq!(result.succeeded, 2);
        assert_eq!(result.failed, 0);

        for id in &ids {
            let row = tracking::get_tracking(&pool, id)
                .await
                .expect("get")
                .unwrap();
            assert_eq!(row.core_status, "in_progress");
            assert!(row.started_at.is_some(), "started_at stamped");
            assert!(row.finished_at.is_none());
        }
        pool.close().await;
        cleanup(&path);
    }

    #[tokio::test]
    async fn set_status_marks_completed_and_stamps_finish() {
        let (pool, path) = migrated_pool("bulk_status_complete.db").await;
        let ids = seed_media(&pool, 1).await;
        let service = BulkService::new(pool.clone());

        let result = service
            .set_status(None, &ids, "completed")
            .await
            .expect("complete");
        assert_eq!(result.succeeded, 1);
        let row = tracking::get_tracking(&pool, &ids[0])
            .await
            .expect("get")
            .unwrap();
        assert_eq!(row.core_status, "completed");
        assert!(row.finished_at.is_some());
        pool.close().await;
        cleanup(&path);
    }

    #[tokio::test]
    async fn set_status_updates_existing_row() {
        let (pool, path) = migrated_pool("bulk_status_update.db").await;
        let ids = seed_media(&pool, 1).await;
        let service = BulkService::new(pool.clone());

        service
            .set_status(None, &ids, "in_progress")
            .await
            .expect("start");
        service
            .set_status(None, &ids, "completed")
            .await
            .expect("complete");
        let row = tracking::get_tracking(&pool, &ids[0])
            .await
            .expect("get")
            .unwrap();
        assert_eq!(row.core_status, "completed");
        assert!(row.finished_at.is_some(), "finish stamped on update");
        pool.close().await;
        cleanup(&path);
    }

    #[tokio::test]
    async fn set_status_records_repeat_guard_failure_without_writing() {
        let (pool, path) = migrated_pool("bulk_status_repeat.db").await;
        let ids = seed_media(&pool, 1).await;
        let service = BulkService::new(pool.clone());

        let result = service
            .set_status(None, &ids, "repeat")
            .await
            .expect("repeat guarded per-media");
        assert_eq!(result.total, 1);
        assert_eq!(result.succeeded, 0);
        assert_eq!(result.failed, 1);
        assert_eq!(result.failures[0].media_id, ids[0]);
        assert!(
            result.failures[0].reason.contains("repeat"),
            "reason explains the guard: {}",
            result.failures[0].reason
        );
        assert!(
            tracking::get_tracking(&pool, &ids[0])
                .await
                .expect("get")
                .is_none(),
            "failed transition writes nothing"
        );
        pool.close().await;
        cleanup(&path);
    }

    #[tokio::test]
    async fn set_status_summarizes_partial_failures_and_keeps_successes() {
        let (pool, path) = migrated_pool("bulk_status_partial.db").await;
        let ids = seed_media(&pool, 2).await;
        let service = BulkService::new(pool.clone());
        service
            .set_status(None, &ids[1..], "completed")
            .await
            .expect("complete second");

        let result = service
            .set_status(None, &ids, "repeat")
            .await
            .expect("partial batch resolves");
        assert_eq!(result.total, 2);
        assert_eq!(result.succeeded, 1);
        assert_eq!(result.failed, 1);
        assert_eq!(
            result.failures[0].media_id, ids[0],
            "planned media cannot repeat"
        );
        assert!(
            tracking::get_tracking(&pool, &ids[0])
                .await
                .expect("get")
                .is_none(),
            "failed media untouched"
        );
        let second = tracking::get_tracking(&pool, &ids[1])
            .await
            .expect("get")
            .expect("tracking row");
        assert_eq!(second.core_status, "repeat");
        pool.close().await;
        cleanup(&path);
    }

    #[tokio::test]
    async fn set_status_applies_to_the_filtered_selection() {
        let (pool, path) = migrated_pool("bulk_status_filter.db").await;
        let media_service = MediaService::new(pool.clone());
        let novel = media_service
            .add_media(input_type("Novel", "novel"))
            .await
            .expect("add novel");
        let anime = media_service
            .add_media(input_type("Anime", "anime"))
            .await
            .expect("add anime");
        let service = BulkService::new(pool.clone());

        let result = service
            .set_status(Some(filter("anime")), &[], "in_progress")
            .await
            .expect("filtered set");
        assert_eq!(result.total, 1);
        assert_eq!(result.succeeded, 1);
        assert_eq!(result.failed, 0);

        assert!(
            tracking::get_tracking(&pool, novel.as_str())
                .await
                .expect("get")
                .is_none(),
            "non-matching media untouched"
        );
        let row = tracking::get_tracking(&pool, anime.as_str())
            .await
            .expect("get")
            .expect("tracking row");
        assert_eq!(row.core_status, "in_progress");
        pool.close().await;
        cleanup(&path);
    }

    #[tokio::test]
    async fn set_status_empty_filter_resolves_to_no_targets() {
        let (pool, path) = migrated_pool("bulk_status_empty_filter.db").await;
        let ids = seed_media(&pool, 1).await;
        let service = BulkService::new(pool.clone());

        let result = service
            .set_status(Some(filter("manga")), &[], "in_progress")
            .await
            .expect("empty set resolves");
        assert_eq!(result.total, 0);
        assert_eq!(result.succeeded, 0);
        assert!(
            tracking::get_tracking(&pool, &ids[0])
                .await
                .expect("get")
                .is_none(),
            "nothing written for an empty selection"
        );
        pool.close().await;
        cleanup(&path);
    }

    #[tokio::test]
    async fn set_status_rejects_unknown_status() {
        let (pool, path) = migrated_pool("bulk_status_bad.db").await;
        let service = BulkService::new(pool.clone());
        let err = service
            .set_status(None, &["m-1".to_string()], "watching")
            .await
            .expect_err("unknown status");
        assert!(matches!(err, AppError::Validation(_)));
        pool.close().await;
        cleanup(&path);
    }

    #[tokio::test]
    async fn add_tag_creates_personal_tag_and_links() {
        let (pool, path) = migrated_pool("bulk_tag_create.db").await;
        let ids = seed_media(&pool, 2).await;
        let service = BulkService::new(pool.clone());

        let result = service.add_tag(None, &ids, "Backlog").await.expect("tag");
        assert_eq!(result.total, 2);
        assert_eq!(result.succeeded, 2);
        assert_eq!(result.failed, 0);

        let tag_id = media::resolve_personal_tag(&pool, "Backlog")
            .await
            .expect("resolve")
            .expect("tag exists");
        let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM media_tag WHERE tag_id = ?")
            .bind(&tag_id)
            .fetch_one(&pool)
            .await
            .expect("count");
        assert_eq!(n, 2);
        pool.close().await;
        cleanup(&path);
    }

    #[tokio::test]
    async fn add_tag_applies_to_the_filtered_selection() {
        let (pool, path) = migrated_pool("bulk_tag_filter.db").await;
        let media_service = MediaService::new(pool.clone());
        let novel = media_service
            .add_media(input_type("Novel", "novel"))
            .await
            .expect("add novel");
        let anime = media_service
            .add_media(input_type("Anime", "anime"))
            .await
            .expect("add anime");
        let service = BulkService::new(pool.clone());

        let result = service
            .add_tag(Some(filter("anime")), &[], "Backlog")
            .await
            .expect("filtered tag");
        assert_eq!(result.total, 1);
        assert_eq!(result.succeeded, 1);

        let tag_id = media::resolve_personal_tag(&pool, "Backlog")
            .await
            .expect("resolve")
            .expect("tag exists");
        let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM media_tag WHERE tag_id = ?")
            .bind(&tag_id)
            .fetch_one(&pool)
            .await
            .expect("count");
        assert_eq!(n, 1, "only the matching media is tagged");
        assert_eq!(
            media::media_tags(&pool, novel.as_str())
                .await
                .expect("tags")
                .len(),
            0,
            "non-matching media untouched"
        );
        assert_eq!(
            media::media_tags(&pool, anime.as_str())
                .await
                .expect("tags")
                .len(),
            1
        );
        pool.close().await;
        cleanup(&path);
    }

    #[tokio::test]
    async fn add_tag_reuses_existing_tag_and_normalizes_name() {
        let (pool, path) = migrated_pool("bulk_tag_reuse.db").await;
        let ids = seed_media(&pool, 2).await;
        let service = BulkService::new(pool.clone());

        service
            .add_tag(None, &ids, "  To   Read ")
            .await
            .expect("tag");
        service
            .add_tag(None, &ids[..1], "To Read")
            .await
            .expect("tag again");

        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tag WHERE scope = 'personal'")
            .fetch_one(&pool)
            .await
            .expect("count");
        assert_eq!(count, 1, "same normalized name reuses the row");
        pool.close().await;
        cleanup(&path);
    }

    #[tokio::test]
    async fn add_tag_rejects_blank_tag() {
        let (pool, path) = migrated_pool("bulk_tag_blank.db").await;
        let service = BulkService::new(pool.clone());
        let err = service
            .add_tag(None, &["m-1".to_string()], "   ")
            .await
            .expect_err("blank tag");
        assert!(matches!(err, AppError::Validation(_)));
        pool.close().await;
        cleanup(&path);
    }

    #[tokio::test]
    async fn delete_moves_many_to_trash() {
        let (pool, path) = migrated_pool("bulk_delete.db").await;
        let ids = seed_media(&pool, 3).await;
        let service = BulkService::new(pool.clone());

        let result = service.delete(None, &ids).await.expect("delete");
        assert_eq!(result.summary.succeeded, 3);
        assert_eq!(result.summary.failed, 0);
        assert_eq!(result.trash_ids.len(), 3);
        assert!(result.trash_ids.iter().all(|id| id.starts_with("t-")));

        let media_service = MediaService::new(pool.clone());
        let remaining = media_service
            .list_media(MediaListInput::default())
            .await
            .expect("list");
        assert!(remaining.is_empty(), "all media soft-deleted");
        pool.close().await;
        cleanup(&path);
    }

    #[tokio::test]
    async fn delete_applies_to_the_filtered_selection() {
        let (pool, path) = migrated_pool("bulk_delete_filter.db").await;
        let media_service = MediaService::new(pool.clone());
        let novel = media_service
            .add_media(input_type("Novel", "novel"))
            .await
            .expect("add novel");
        media_service
            .add_media(input_type("Anime", "anime"))
            .await
            .expect("add anime");
        let service = BulkService::new(pool.clone());

        let result = service
            .delete(Some(filter("anime")), &[])
            .await
            .expect("delete");
        assert_eq!(result.summary.total, 1);
        assert_eq!(result.summary.succeeded, 1);
        assert_eq!(result.trash_ids.len(), 1);

        let remaining = media_service
            .list_media(MediaListInput::default())
            .await
            .expect("list");
        let ids: Vec<&str> = remaining.iter().map(|item| item.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![novel.as_str()],
            "only the matching media is deleted"
        );
        pool.close().await;
        cleanup(&path);
    }

    #[tokio::test]
    async fn delete_records_unknown_media_and_undo_restores_only_successes() {
        let (pool, path) = migrated_pool("bulk_delete_unknown.db").await;
        let ids = seed_media(&pool, 1).await;
        let service = BulkService::new(pool.clone());

        let mut targets = ids.clone();
        targets.push("m-nope".to_string());
        let result = service.delete(None, &targets).await.expect("delete");
        assert_eq!(result.summary.total, 2);
        assert_eq!(result.summary.succeeded, 1);
        assert_eq!(result.summary.failed, 1);
        assert_eq!(result.summary.failures[0].media_id, "m-nope");
        assert_eq!(
            result.trash_ids.len(),
            1,
            "undo restores exactly the successes"
        );
        pool.close().await;
        cleanup(&path);
    }

    #[tokio::test]
    async fn set_status_logs_activity_per_media() {
        let (pool, path) = migrated_pool("activity_bulk_status.db").await;
        let ids = seed_media(&pool, 2).await;
        let service = BulkService::new(pool.clone());

        service
            .set_status(None, &ids, "in_progress")
            .await
            .expect("set status");

        for id in &ids {
            let entries = activity::list_for_media(&pool, id, 10).await.expect("list");
            let kinds: Vec<&str> = entries.iter().map(|e| e.kind.as_str()).collect();
            assert_eq!(kinds, vec!["started"], "bulk transitions log per media");
        }
        pool.close().await;
        cleanup(&path);
    }

    fn cleanup(path: &std::path::Path) {
        crate::infrastructure::test_support::cleanup_files(path);
    }
}
