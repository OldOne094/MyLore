//! Collection use-cases (MISSION-076).
//!
//! CRUD over the `collection` rows plus ordered membership: media are added,
//! removed, and reordered inside a collection through the `position` column on
//! `collection_member`. `reorder` replaces the whole ordered member set in one
//! transaction (repo `set_members`) so a drag/drop reorder commits atomically.

use std::collections::HashMap;

use chrono::Utc;
use sqlx::SqlitePool;
use tracing::info;
use uuid::Uuid;

use crate::application::media_service::{MediaListItem, MediaService};
use crate::error::AppError;
use crate::infrastructure::repositories::collection;

/// A collection row surfaced to the Collections page and the add-to-list picker.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CollectionView {
    pub id: String,
    pub name: String,
    pub member_count: i64,
    pub created_at: String,
}

/// One media inside a collection, with its ordered position.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CollectionMemberView {
    pub position: i64,
    pub media: MediaListItem,
}

/// Collection use-cases.
pub struct CollectionService {
    pool: SqlitePool,
}

impl CollectionService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Create a manual collection; resolves with its view (0 members).
    pub async fn create(&self, name: &str) -> Result<CollectionView, AppError> {
        let name = normalize_name(name)?;
        info!(name, "collection_create");
        let now = Utc::now().to_rfc3339();
        let id = format!("c-{}", Uuid::new_v4());
        let sort_order = collection::list(&self.pool).await?.len() as i64;
        collection::create(
            &self.pool,
            &collection::CollectionRecord {
                id: id.clone(),
                name: name.clone(),
                is_smart: false,
                filter_def: None,
                sort_order,
                created_at: now.clone(),
            },
        )
        .await?;
        Ok(CollectionView {
            id,
            name,
            member_count: 0,
            created_at: now,
        })
    }

    /// Rename a collection; resolves with the updated view.
    pub async fn rename(&self, id: &str, name: &str) -> Result<CollectionView, AppError> {
        let name = normalize_name(name)?;
        let mut record = self.require(id).await?;
        info!(id, name, "collection_rename");
        record.name = name;
        collection::update(&self.pool, &record).await?;
        self.view(id).await
    }

    /// Delete a collection; members cascade. Resolves with the removed name.
    pub async fn delete(&self, id: &str) -> Result<String, AppError> {
        let record = self.require(id).await?;
        info!(id, "collection_delete");
        collection::delete(&self.pool, id).await?;
        Ok(record.name)
    }

    /// All collections with member counts, ordered by `sort_order` then name.
    pub async fn list(&self) -> Result<Vec<CollectionView>, AppError> {
        let rows = collection::list_with_counts(&self.pool).await?;
        Ok(rows
            .into_iter()
            .map(|(record, count)| CollectionView {
                id: record.id,
                name: record.name,
                member_count: count,
                created_at: record.created_at,
            })
            .collect())
    }

    /// A collection's members in display order.
    pub async fn members(&self, id: &str) -> Result<Vec<CollectionMemberView>, AppError> {
        self.require(id).await?;
        let joined = collection::members_with_media(&self.pool, id).await?;
        let rows: Vec<_> = joined.iter().map(|(_, summary)| summary.clone()).collect();
        let media = MediaService::new(self.pool.clone())
            .to_list_items(rows)
            .await?;
        Ok(joined
            .iter()
            .zip(media)
            .map(|((member, _), item)| CollectionMemberView {
                position: member.position,
                media: item,
            })
            .collect())
    }

    /// Append many media to a collection (idempotent — existing members keep
    /// their row, new ones land after the current tail). MISSION-045 bulk add.
    pub async fn add_members(&self, id: &str, media_ids: &[String]) -> Result<(), AppError> {
        self.require(id).await?;
        let base = collection::members(&self.pool, id).await?.len() as i64;
        let added_at = Utc::now().to_rfc3339();
        for (index, media_id) in media_ids.iter().enumerate() {
            collection::add_member(&self.pool, id, media_id, base + index as i64, &added_at)
                .await?;
        }
        Ok(())
    }

    /// Remove one media from a collection, renumbering the tail so positions
    /// stay contiguous. Removing a non-member is a no-op.
    pub async fn remove_member(&self, id: &str, media_id: &str) -> Result<(), AppError> {
        self.require(id).await?;
        let current = collection::members(&self.pool, id).await?;
        let rebuilt: Vec<_> = current
            .into_iter()
            .filter(|m| m.media_id != media_id)
            .enumerate()
            .map(|(position, m)| collection::CollectionMember {
                collection_id: id.to_string(),
                media_id: m.media_id,
                position: position as i64,
                added_at: m.added_at,
            })
            .collect();
        collection::set_members(&self.pool, id, &rebuilt).await?;
        Ok(())
    }

    /// Persist a drag/drop reorder: the given media ids must be exactly the
    /// collection's current members (same set, any order); their positions are
    /// rewritten 0..n atomically.
    pub async fn reorder(&self, id: &str, ordered_ids: &[String]) -> Result<(), AppError> {
        self.require(id).await?;
        let current = collection::members(&self.pool, id).await?;
        let existing: Vec<String> = current.iter().map(|m| m.media_id.clone()).collect();
        let mut expected = existing.clone();
        expected.sort();
        let mut given = ordered_ids.to_vec();
        given.sort();
        if given != expected {
            return Err(AppError::validation(
                "reorder must include exactly the collection's current members",
            ));
        }
        let added_at: HashMap<&str, &str> = current
            .iter()
            .map(|m| (m.media_id.as_str(), m.added_at.as_str()))
            .collect();
        let rebuilt = ordered_ids
            .iter()
            .enumerate()
            .map(|(position, media_id)| collection::CollectionMember {
                collection_id: id.to_string(),
                media_id: media_id.clone(),
                position: position as i64,
                added_at: added_at
                    .get(media_id.as_str())
                    .copied()
                    .unwrap_or_default()
                    .to_string(),
            })
            .collect::<Vec<_>>();
        collection::set_members(&self.pool, id, &rebuilt).await?;
        Ok(())
    }

    /// A single collection view (must exist).
    async fn view(&self, id: &str) -> Result<CollectionView, AppError> {
        let count = collection::count_members(&self.pool, id).await?;
        let record = self.require(id).await?;
        Ok(CollectionView {
            id: record.id,
            name: record.name,
            member_count: count,
            created_at: record.created_at,
        })
    }

    async fn require(&self, id: &str) -> Result<collection::CollectionRecord, AppError> {
        collection::get(&self.pool, id)
            .await?
            .ok_or_else(|| AppError::validation(format!("collection not found: {id}")))
    }
}

fn normalize_name(name: &str) -> Result<String, AppError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::validation("collection name must not be empty"));
    }
    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::test_support::{cleanup_files, migrated_pool};

    async fn seed_media(pool: &SqlitePool, ids: &[&str]) {
        for id in ids {
            sqlx::query(
                "INSERT INTO media (id, content_type, title_main, created_at, updated_at)
                 VALUES (?, 'novel', 'Title', '2026-01-01', '2026-01-01')",
            )
            .bind(id)
            .execute(pool)
            .await
            .expect("seed media");
        }
    }

    #[tokio::test]
    async fn create_lists_and_renames() {
        let (pool, path) = migrated_pool("collection_service_crud.db").await;
        let service = CollectionService::new(pool.clone());

        let created = service.create("  Reading Now  ").await.expect("create");
        assert_eq!(created.name, "Reading Now", "name is trimmed");
        assert_eq!(created.member_count, 0);
        let id = created.id.clone();

        let renamed = service.rename(&id, "Reading Later").await.expect("rename");
        assert_eq!(renamed.name, "Reading Later");

        let rows = service.list().await.expect("list");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "Reading Later");
        assert_eq!(rows[0].id, id);

        let removed = service.delete(&id).await.expect("delete");
        assert_eq!(removed, "Reading Later");
        assert!(service.list().await.expect("list").is_empty());
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn create_rejects_blank_name() {
        let (pool, path) = migrated_pool("collection_service_blank.db").await;
        let service = CollectionService::new(pool.clone());
        let err = service.create("   ").await.expect_err("blank name");
        assert!(matches!(err, AppError::Validation(_)));
        let err = service.rename("c-1", "").await.expect_err("empty rename");
        assert!(matches!(err, AppError::Validation(_)));
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn members_roundtrip_add_remove_and_reorder() {
        let (pool, path) = migrated_pool("collection_service_members.db").await;
        seed_media(&pool, &["m-1", "m-2", "m-3"]).await;
        let service = CollectionService::new(pool.clone());
        let id = service.create("Shelf").await.expect("create").id;

        service
            .add_members(&id, &["m-1".into(), "m-2".into()])
            .await
            .expect("add m1,m2");
        service
            .add_members(&id, &["m-3".into()])
            .await
            .expect("add m3");

        let members = service.members(&id).await.expect("members");
        assert_eq!(members.len(), 3);
        assert_eq!(members[0].position, 0);
        assert_eq!(members[0].media.id, "m-1");
        assert_eq!(members[2].media.id, "m-3", "appended after the tail");

        // Reorder: m-3, m-1, m-2.
        service
            .reorder(&id, &["m-3".into(), "m-1".into(), "m-2".into()])
            .await
            .expect("reorder");
        let members = service.members(&id).await.expect("members");
        assert_eq!(members[0].media.id, "m-3");
        assert_eq!(members[1].media.id, "m-1");
        assert_eq!(members[2].media.id, "m-2");

        // A reorder that doesn't match the member set is rejected and writes nothing.
        let err = service
            .reorder(&id, &["m-3".into(), "m-1".into()])
            .await
            .expect_err("wrong set");
        assert!(matches!(err, AppError::Validation(_)));
        let members = service.members(&id).await.expect("members unchanged");
        assert_eq!(members.len(), 3);

        // Remove the middle member; the tail renumbers.
        service.remove_member(&id, "m-1").await.expect("remove m1");
        let members = service.members(&id).await.expect("members after remove");
        assert_eq!(members.len(), 2);
        assert_eq!(members[0].media.id, "m-3");
        assert_eq!(members[0].position, 0, "positions stay contiguous");
        assert_eq!(members[1].media.id, "m-2");
        assert_eq!(members[1].position, 1);

        // Removing a media that isn't a member is a no-op.
        service
            .remove_member(&id, "m-missing")
            .await
            .expect("no-op remove");
        assert_eq!(service.members(&id).await.expect("members").len(), 2);

        let view = service.list().await.expect("list");
        assert_eq!(view[0].member_count, 2);
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn operations_reject_unknown_collection() {
        let (pool, path) = migrated_pool("collection_service_missing.db").await;
        let service = CollectionService::new(pool.clone());

        let err = service.rename("c-nope", "X").await.expect_err("rename");
        assert!(matches!(err, AppError::Validation(_)));
        let err = service.delete("c-nope").await.expect_err("delete");
        assert!(matches!(err, AppError::Validation(_)));
        let err = service.members("c-nope").await.expect_err("members");
        assert!(matches!(err, AppError::Validation(_)));
        let err = service
            .add_members("c-nope", &["m-1".into()])
            .await
            .expect_err("add");
        assert!(matches!(err, AppError::Validation(_)));
        let err = service
            .remove_member("c-nope", "m-1")
            .await
            .expect_err("remove");
        assert!(matches!(err, AppError::Validation(_)));
        let err = service.reorder("c-nope", &[]).await.expect_err("reorder");
        assert!(matches!(err, AppError::Validation(_)));
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn members_carry_progress_and_favorite_via_media_dto() {
        let (pool, path) = migrated_pool("collection_service_dto.db").await;
        sqlx::query(
            "INSERT INTO media (id, content_type, title_main, created_at, updated_at)
             VALUES ('m-1', 'anime', 'Steins;Gate', '2026-01-01', '2026-01-01')",
        )
        .execute(&pool)
        .await
        .expect("seed media");
        sqlx::query(
            "INSERT INTO review (media_id, rating, review, favorite, is_spoiler, created_at, updated_at)
             VALUES ('m-1', NULL, NULL, 1, 0, '2026-01-01', '2026-01-01')",
        )
        .execute(&pool)
        .await
        .expect("seed review");
        let service = CollectionService::new(pool.clone());
        let id = service.create("Shelf").await.expect("create").id;
        service
            .add_members(&id, &["m-1".into()])
            .await
            .expect("add");

        let members = service.members(&id).await.expect("members");
        assert_eq!(members[0].media.title, "Steins;Gate");
        assert!(members[0].media.favorite, "favorite rides through");
        assert_eq!(members[0].media.progress.completed, 0, "progress default");
        pool.close().await;
        cleanup_files(&path);
    }
}
