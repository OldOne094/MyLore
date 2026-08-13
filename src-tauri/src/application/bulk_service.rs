//! Bulk operations application service (MISSION-045).
//!
//! Actions behind the library action bar: set tracking status, add a personal
//! tag, soft-delete (to trash), and add to a collection. All four operate on a
//! set of media ids selected in the UI. MISSION-077 extends this with
//! filtered-selection bulk ops that carry a change summary.
//!
//! Status uses the domain status engine per media, so stamps (started_at /
//! finished_at / repeat_count) stay correct and the Repeat-guard still applies.

use std::str::FromStr;

use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::application::trash_service::TrashService;
use crate::domain::enums::CoreStatus;
use crate::domain::status::apply_transition;
use crate::domain::tracking::Tracking;
use crate::domain::value_objects::{DateOnly, MediaId};
use crate::error::AppError;
use crate::infrastructure::repositories::{collection, media, tracking};

/// A collection row surfaced to the "add to list" picker.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CollectionItem {
    pub id: String,
    pub name: String,
}

/// Bulk use-cases over a set of media ids.
pub struct BulkService {
    pool: SqlitePool,
}

impl BulkService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Set the tracking status for many media at once. Media without a tracking
    /// row start from `Planned`; every media moves through the status engine so
    /// side-effect stamps stay correct. Rejects with the offending media id when
    /// any media cannot move to the target (e.g. Repeat without consumption).
    pub async fn set_status(&self, ids: &[String], core_status: &str) -> Result<(), AppError> {
        let to = CoreStatus::from_str(core_status)?;
        let today = DateOnly::new(Utc::now().format("%Y-%m-%d").to_string())?;
        let updated_at = Utc::now().to_rfc3339();

        for raw_id in ids {
            let media_id = MediaId::new(raw_id)?;
            let existing = tracking::get_tracking(&self.pool, media_id.as_str()).await?;
            let current = existing_to_domain(existing, &media_id, &updated_at)?;
            let next = apply_transition(&current, to, &today)
                .map_err(|err| AppError::validation(format!("{}: {}", media_id.as_str(), err)))?;
            let mut next = next;
            next.updated_at = updated_at.clone();
            tracking::upsert_tracking(&self.pool, &domain_to_record(&next)).await?;
        }
        Ok(())
    }

    /// Add a personal tag to many media. The tag row is reused when one with the
    /// same name already exists, else created (scope `personal`); media already
    /// carrying the tag are skipped.
    pub async fn add_tag(&self, ids: &[String], tag: &str) -> Result<(), AppError> {
        let name = normalize_tag(tag)?;
        let tag_id = match media::resolve_personal_tag(&self.pool, &name).await? {
            Some(id) => id,
            None => {
                let id = format!("tag-{}", Uuid::new_v4());
                media::create_personal_tag(&self.pool, &id, &name).await?;
                id
            }
        };
        media::add_tag_to_many(&self.pool, &tag_id, ids).await?;
        Ok(())
    }

    /// Soft-delete many media. Resolves with one trash id per media so the UI
    /// can offer a single "undo" that restores the whole batch.
    pub async fn delete(&self, ids: &[String]) -> Result<Vec<String>, AppError> {
        let trash_service = TrashService::new(self.pool.clone());
        let mut trash_ids = Vec::with_capacity(ids.len());
        for id in ids {
            trash_ids.push(trash_service.delete_media(id).await?);
        }
        Ok(trash_ids)
    }

    /// List collections for the "add to list" picker.
    pub async fn list_collections(&self) -> Result<Vec<CollectionItem>, AppError> {
        let rows = collection::list(&self.pool).await?;
        Ok(rows
            .into_iter()
            .map(|row| CollectionItem {
                id: row.id,
                name: row.name,
            })
            .collect())
    }

    /// Add many media to one collection. The collection must exist; members are
    /// appended after the existing ones (existing rows just update position).
    pub async fn add_to_list(
        &self,
        collection_id: &str,
        media_ids: &[String],
    ) -> Result<(), AppError> {
        if collection::get(&self.pool, collection_id).await?.is_none() {
            return Err(AppError::validation(format!(
                "collection not found: {collection_id}"
            )));
        }
        let added_at = Utc::now().to_rfc3339();
        let base = collection::members(&self.pool, collection_id).await?.len() as i64;
        for (index, media_id) in media_ids.iter().enumerate() {
            collection::add_member(
                &self.pool,
                collection_id,
                media_id,
                base + index as i64,
                &added_at,
            )
            .await?;
        }
        Ok(())
    }
}

/// Trim and collapse internal whitespace on a tag name (display keeps casing).
fn normalize_tag(tag: &str) -> Result<String, AppError> {
    let collapsed = tag.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return Err(AppError::validation("tag must not be empty"));
    }
    Ok(collapsed)
}

/// Map a persisted tracking row (or nothing) into the domain aggregate. A
/// missing row becomes a fresh `Planned` record; the status engine then moves
/// it to the requested status.
fn existing_to_domain(
    record: Option<tracking::TrackingRecord>,
    media_id: &MediaId,
    updated_at: &str,
) -> Result<Tracking, AppError> {
    let Some(record) = record else {
        return Ok(Tracking {
            media_id: media_id.clone(),
            core_status: CoreStatus::Planned,
            custom_status_id: None,
            started_at: None,
            finished_at: None,
            repeat_count: 0,
            current_node_id: None,
            current_position: None,
            updated_at: updated_at.to_string(),
        });
    };

    let core_status = CoreStatus::from_str(&record.core_status)?;
    let started_at = record
        .started_at
        .as_deref()
        .map(DateOnly::new)
        .transpose()?;
    let finished_at = record
        .finished_at
        .as_deref()
        .map(DateOnly::new)
        .transpose()?;
    let repeat_count = u32::try_from(record.repeat_count)
        .map_err(|_| AppError::validation("tracking repeat_count out of range"))?;
    let current_position = record
        .current_position
        .map(|p| {
            u32::try_from(p).map_err(|_| AppError::validation("tracking position out of range"))
        })
        .transpose()?;

    Ok(Tracking {
        media_id: media_id.clone(),
        core_status,
        custom_status_id: record.custom_status_id,
        started_at,
        finished_at,
        repeat_count,
        current_node_id: record.current_node_id,
        current_position,
        updated_at: record.updated_at,
    })
}

/// Persist a domain aggregate as a tracking record.
fn domain_to_record(next: &Tracking) -> tracking::TrackingRecord {
    tracking::TrackingRecord {
        media_id: next.media_id.as_str().to_string(),
        core_status: next.core_status.as_str().to_string(),
        custom_status_id: next.custom_status_id.clone(),
        started_at: next.started_at.as_ref().map(|d| d.as_str().to_string()),
        finished_at: next.finished_at.as_ref().map(|d| d.as_str().to_string()),
        repeat_count: i64::from(next.repeat_count),
        current_node_id: next.current_node_id.clone(),
        current_position: next.current_position.map(i64::from),
        updated_at: next.updated_at.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::media_service::{AddMediaInput, MediaListInput, MediaService};
    use crate::infrastructure::repositories::collection::CollectionRecord;
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

        service
            .set_status(&ids, "in_progress")
            .await
            .expect("set status");

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

        service
            .set_status(&ids, "completed")
            .await
            .expect("complete");
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
            .set_status(&ids, "in_progress")
            .await
            .expect("start");
        service
            .set_status(&ids, "completed")
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
    async fn set_status_rejects_repeat_without_consumption() {
        let (pool, path) = migrated_pool("bulk_status_repeat.db").await;
        let ids = seed_media(&pool, 1).await;
        let service = BulkService::new(pool.clone());

        let err = service
            .set_status(&ids, "repeat")
            .await
            .expect_err("repeat guard");
        assert!(matches!(err, AppError::Validation(_)));
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
    async fn set_status_rejects_unknown_status() {
        let (pool, path) = migrated_pool("bulk_status_bad.db").await;
        let service = BulkService::new(pool.clone());
        let err = service
            .set_status(&["m-1".to_string()], "watching")
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

        service.add_tag(&ids, "Backlog").await.expect("tag");

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
    async fn add_tag_reuses_existing_tag_and_normalizes_name() {
        let (pool, path) = migrated_pool("bulk_tag_reuse.db").await;
        let ids = seed_media(&pool, 2).await;
        let service = BulkService::new(pool.clone());

        service.add_tag(&ids, "  To   Read ").await.expect("tag");
        service
            .add_tag(&ids[..1], "To Read")
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
            .add_tag(&["m-1".to_string()], "   ")
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

        let trash_ids = service.delete(&ids).await.expect("delete");
        assert_eq!(trash_ids.len(), 3);
        assert!(trash_ids.iter().all(|id| id.starts_with("t-")));

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
    async fn list_collections_returns_rows() {
        let (pool, path) = migrated_pool("bulk_list_collections.db").await;
        let service = BulkService::new(pool.clone());
        collection::create(
            &pool,
            &CollectionRecord {
                id: "c-1".into(),
                name: "Reading Now".into(),
                is_smart: false,
                filter_def: None,
                sort_order: 0,
                created_at: "2026-01-01".into(),
            },
        )
        .await
        .expect("create");

        let rows = service.list_collections().await.expect("list");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "Reading Now");
        pool.close().await;
        cleanup(&path);
    }

    #[tokio::test]
    async fn add_to_list_appends_members() {
        let (pool, path) = migrated_pool("bulk_add_to_list.db").await;
        let ids = seed_media(&pool, 2).await;
        let service = BulkService::new(pool.clone());
        collection::create(
            &pool,
            &CollectionRecord {
                id: "c-1".into(),
                name: "Reading Now".into(),
                is_smart: false,
                filter_def: None,
                sort_order: 0,
                created_at: "2026-01-01".into(),
            },
        )
        .await
        .expect("create");

        service.add_to_list("c-1", &ids).await.expect("add");

        let members = collection::members(&pool, "c-1").await.expect("members");
        assert_eq!(members.len(), 2);
        assert_eq!(members[0].media_id, ids[0]);
        assert_eq!(members[1].media_id, ids[1]);
        pool.close().await;
        cleanup(&path);
    }

    #[tokio::test]
    async fn add_to_list_rejects_unknown_collection() {
        let (pool, path) = migrated_pool("bulk_add_to_list_bad.db").await;
        let service = BulkService::new(pool.clone());
        let err = service
            .add_to_list("c-nope", &["m-1".to_string()])
            .await
            .expect_err("unknown collection");
        assert!(matches!(err, AppError::Validation(_)));
        pool.close().await;
        cleanup(&path);
    }

    fn cleanup(path: &std::path::Path) {
        crate::infrastructure::test_support::cleanup_files(path);
    }
}
