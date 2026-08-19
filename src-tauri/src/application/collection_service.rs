//! Collection use-cases (MISSION-076, MISSION-077).
//!
//! CRUD over the `collection` rows plus ordered membership: media are added,
//! removed, and reordered inside a collection through the `position` column on
//! `collection_member`. `reorder` replaces the whole ordered member set in one
//! transaction (repo `set_members`) so a drag/drop reorder commits atomically.
//!
//! **Smart collections (MISSION-077)** store a serialized `SmartFilter` in
//! `collection.filter_def` (`is_smart = 1`). Their membership is computed on
//! demand by re-running the filter against the media repo (`list`/`count`), so
//! they update automatically as the library changes; manual membership ops
//! (`add_members`/`remove_member`/`reorder`) reject smart collections.

use std::collections::HashMap;

use chrono::Utc;
use sqlx::SqlitePool;
use tracing::info;
use uuid::Uuid;

use crate::application::bulk_service::{BulkFailure, BulkResult};
use crate::application::media_service::{MediaListItem, MediaService};
use crate::error::AppError;
use crate::infrastructure::repositories::collection;
use crate::infrastructure::repositories::media::{self as media_repo, MediaFilter, MediaSort};

/// A collection row surfaced to the Collections page and the add-to-list picker.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CollectionView {
    pub id: String,
    pub name: String,
    /// Whether members are stored manually (`false`) or computed from `filter`
    /// (`true`, MISSION-077).
    pub is_smart: bool,
    /// The saved filter of a smart collection (always `None` for manual ones).
    pub filter: Option<SmartFilter>,
    pub member_count: i64,
    pub created_at: String,
}

/// A saved library filter definition (MISSION-077). Mirrors the flat filter
/// fields `LibraryFilters`/`filtersToArgs` produce on the frontend and the
/// media repo's `MediaFilter`; serialized to JSON in `collection.filter_def`.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SmartFilter {
    pub content_type: Option<String>,
    pub format: Option<String>,
    pub pub_status: Option<String>,
    pub genre: Option<String>,
    pub tag: Option<String>,
    pub year: Option<i64>,
    pub favorite: Option<bool>,
    pub sort: Option<String>,
    pub ascending: Option<bool>,
}

impl SmartFilter {
    /// Convert into the media repo filter; rejects an unknown `sort` value.
    fn to_media_filter(&self) -> Result<MediaFilter, AppError> {
        let sort = match self.sort.as_deref().unwrap_or("title") {
            "title" => MediaSort::Title,
            "created_at" => MediaSort::CreatedAt,
            "updated_at" => MediaSort::UpdatedAt,
            "release_year" => MediaSort::ReleaseYear,
            other => return Err(AppError::validation(format!("unknown sort: {other}"))),
        };
        let ascending = match (self.sort.as_deref(), self.ascending) {
            (None, _) => true,
            (_, Some(ascending)) => ascending,
            (Some(_), None) => false,
        };
        Ok(MediaFilter {
            content_type: self.content_type.clone(),
            format: self.format.clone(),
            pub_status: self.pub_status.clone(),
            genre: self.genre.clone(),
            tag: self.tag.clone(),
            year: self.year,
            favorite: self.favorite,
            search: None,
            sort,
            ascending,
            limit: None,
            offset: None,
        })
    }
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
        self.view(&id).await
    }

    /// Create a smart collection from a saved filter (MISSION-077). Its
    /// membership is computed live, so it starts with however many media match.
    pub async fn create_smart(
        &self,
        name: &str,
        filter: &SmartFilter,
    ) -> Result<CollectionView, AppError> {
        let name = normalize_name(name)?;
        let _ = filter.to_media_filter()?; // reject an unknown sort up front
        info!(name, "collection_create_smart");
        let now = Utc::now().to_rfc3339();
        let id = format!("c-{}", Uuid::new_v4());
        let sort_order = collection::list(&self.pool).await?.len() as i64;
        collection::create(
            &self.pool,
            &collection::CollectionRecord {
                id: id.clone(),
                name: name.clone(),
                is_smart: true,
                filter_def: Some(serde_json::to_string(filter)?),
                sort_order,
                created_at: now.clone(),
            },
        )
        .await?;
        self.view(&id).await
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

    /// Replace a smart collection's filter; resolves with the updated view.
    pub async fn update_smart_filter(
        &self,
        id: &str,
        filter: &SmartFilter,
    ) -> Result<CollectionView, AppError> {
        let _ = filter.to_media_filter()?;
        let mut record = self.require(id).await?;
        if !record.is_smart {
            return Err(AppError::validation(
                "only smart collections have editable filters",
            ));
        }
        info!(id, "collection_update_smart");
        record.filter_def = Some(serde_json::to_string(filter)?);
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
    /// Smart collections report how many media currently match their filter.
    pub async fn list(&self) -> Result<Vec<CollectionView>, AppError> {
        let rows = collection::list_with_counts(&self.pool).await?;
        let mut out = Vec::with_capacity(rows.len());
        for (record, manual_count) in rows {
            let member_count = if record.is_smart {
                self.smart_count(&record).await?
            } else {
                manual_count
            };
            out.push(to_view(record, member_count));
        }
        Ok(out)
    }

    /// A collection's members in display order. Smart collections resolve
    /// their members by re-running their saved filter against the library.
    pub async fn members(&self, id: &str) -> Result<Vec<CollectionMemberView>, AppError> {
        let record = self.require(id).await?;
        if record.is_smart {
            return self.smart_members(&record).await;
        }
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
    /// their row, new ones land after the current tail). Media that cannot be
    /// added (e.g. an unknown id) land in the summary's failures instead of
    /// aborting the batch. MISSION-045 bulk add, MISSION-078 change summary.
    pub async fn add_members(
        &self,
        id: &str,
        media_ids: &[String],
    ) -> Result<BulkResult, AppError> {
        let record = self.require(id).await?;
        ensure_manual(&record)?;
        let base = collection::members(&self.pool, id).await?.len() as i64;
        let added_at = Utc::now().to_rfc3339();
        let mut result = BulkResult {
            total: media_ids.len(),
            succeeded: 0,
            failed: 0,
            failures: Vec::new(),
        };
        for (index, media_id) in media_ids.iter().enumerate() {
            match collection::add_member(&self.pool, id, media_id, base + index as i64, &added_at)
                .await
            {
                Ok(()) => result.succeeded += 1,
                Err(err) => {
                    result.failed += 1;
                    result.failures.push(BulkFailure {
                        media_id: media_id.clone(),
                        reason: err.to_string(),
                    });
                }
            }
        }
        Ok(result)
    }

    /// Remove one media from a collection, renumbering the tail so positions
    /// stay contiguous. Removing a non-member is a no-op.
    pub async fn remove_member(&self, id: &str, media_id: &str) -> Result<(), AppError> {
        let record = self.require(id).await?;
        ensure_manual(&record)?;
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
        let record = self.require(id).await?;
        ensure_manual(&record)?;
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
        let record = self.require(id).await?;
        let member_count = if record.is_smart {
            self.smart_count(&record).await?
        } else {
            collection::count_members(&self.pool, id).await?
        };
        Ok(to_view(record, member_count))
    }

    /// How many media currently match a smart collection's filter.
    async fn smart_count(&self, record: &collection::CollectionRecord) -> Result<i64, AppError> {
        let filter = parse_filter(record.filter_def.as_deref())?.to_media_filter()?;
        media_repo::count(&self.pool, &filter).await
    }

    /// A smart collection's members, resolved live from its saved filter in
    /// media order (position = index).
    async fn smart_members(
        &self,
        record: &collection::CollectionRecord,
    ) -> Result<Vec<CollectionMemberView>, AppError> {
        let filter = parse_filter(record.filter_def.as_deref())?.to_media_filter()?;
        let rows = media_repo::list(&self.pool, &filter).await?;
        let items = MediaService::new(self.pool.clone())
            .to_list_items(rows)
            .await?;
        Ok(items
            .into_iter()
            .enumerate()
            .map(|(position, media)| CollectionMemberView {
                position: position as i64,
                media,
            })
            .collect())
    }

    async fn require(&self, id: &str) -> Result<collection::CollectionRecord, AppError> {
        collection::get(&self.pool, id)
            .await?
            .ok_or_else(|| AppError::validation(format!("collection not found: {id}")))
    }
}

/// Manual-only membership operations are meaningless on a computed collection.
fn ensure_manual(record: &collection::CollectionRecord) -> Result<(), AppError> {
    if record.is_smart {
        return Err(AppError::validation(
            "smart collections are computed from filters",
        ));
    }
    Ok(())
}

/// Decode a stored `filter_def` into a `SmartFilter`; rejects corrupt JSON or
/// a smart collection whose filter is missing.
fn parse_filter(filter_def: Option<&str>) -> Result<SmartFilter, AppError> {
    match filter_def {
        Some(json) => serde_json::from_str(json)
            .map_err(|_| AppError::validation("collection filter is invalid")),
        None => Err(AppError::validation(
            "smart collection is missing its filter",
        )),
    }
}

/// Build the public view; smart collections carry their decoded filter.
fn to_view(record: collection::CollectionRecord, member_count: i64) -> CollectionView {
    let filter = if record.is_smart {
        record
            .filter_def
            .as_deref()
            .and_then(|json| serde_json::from_str(json).ok())
    } else {
        None
    };
    CollectionView {
        id: record.id,
        name: record.name,
        is_smart: record.is_smart,
        filter,
        member_count,
        created_at: record.created_at,
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

    async fn seed_media_row(pool: &SqlitePool, id: &str, content_type: &str, title: &str) {
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

    #[tokio::test]
    async fn add_members_summarizes_unknown_media_failures() {
        let (pool, path) = migrated_pool("collection_service_bulk_partial.db").await;
        seed_media(&pool, &["m-1"]).await;
        let service = CollectionService::new(pool.clone());
        let id = service.create("Shelf").await.expect("create").id;

        let result = service
            .add_members(&id, &["m-1".into(), "m-nope".into()])
            .await
            .expect("partial batch resolves");
        assert_eq!(result.total, 2);
        assert_eq!(result.succeeded, 1);
        assert_eq!(result.failed, 1);
        assert_eq!(result.failures[0].media_id, "m-nope");

        let members = service.members(&id).await.expect("members");
        assert_eq!(members.len(), 1, "only the valid media is a member");
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn smart_collection_resolves_live_members_and_count() {
        let (pool, path) = migrated_pool("collection_service_smart.db").await;
        seed_media_row(&pool, "m-1", "novel", "Dune").await;
        seed_media_row(&pool, "m-2", "anime", "Steins;Gate").await;
        seed_media_row(&pool, "m-3", "anime", "Berserk").await;
        let service = CollectionService::new(pool.clone());

        let view = service
            .create_smart(
                "Anime",
                &SmartFilter {
                    content_type: Some("anime".into()),
                    ..Default::default()
                },
            )
            .await
            .expect("create smart");

        assert!(view.is_smart, "flagged as smart");
        assert_eq!(view.member_count, 2, "count computed from the filter");
        assert_eq!(
            view.filter,
            Some(SmartFilter {
                content_type: Some("anime".into()),
                ..Default::default()
            }),
            "view carries the decoded filter"
        );

        let members = service.members(&view.id).await.expect("smart members");
        let titles: Vec<&str> = members.iter().map(|m| m.media.title.as_str()).collect();
        assert_eq!(titles, vec!["Berserk", "Steins;Gate"], "title-ascending");
        assert_eq!(members[0].position, 0);
        assert_eq!(members[1].position, 1);

        let listed = service.list().await.expect("list");
        assert_eq!(listed[0].member_count, 2);
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn smart_members_follow_filter_updates_and_favorite_flag() {
        let (pool, path) = migrated_pool("collection_service_smart_update.db").await;
        seed_media_row(&pool, "m-1", "novel", "Dune").await;
        seed_media_row(&pool, "m-2", "anime", "Steins;Gate").await;
        sqlx::query(
            "INSERT INTO review (media_id, rating, review, favorite, is_spoiler, created_at, updated_at)
             VALUES ('m-2', NULL, NULL, 1, 0, '2026-01-01', '2026-01-01')",
        )
        .execute(&pool)
        .await
        .expect("seed favorite");
        let service = CollectionService::new(pool.clone());
        let id = service
            .create_smart(
                "Books",
                &SmartFilter {
                    content_type: Some("novel".into()),
                    ..Default::default()
                },
            )
            .await
            .expect("create")
            .id;
        assert_eq!(service.view(&id).await.unwrap().member_count, 1);

        let updated = service
            .update_smart_filter(
                &id,
                &SmartFilter {
                    favorite: Some(true),
                    ..Default::default()
                },
            )
            .await
            .expect("update filter");
        assert_eq!(updated.member_count, 1, "only the favorited media matches");

        let members = service.members(&id).await.expect("members");
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].media.id, "m-2");
        assert!(members[0].media.favorite, "flag rides through smart rows");
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn manual_ops_reject_smart_collections() {
        let (pool, path) = migrated_pool("collection_service_smart_guard.db").await;
        seed_media(&pool, &["m-1"]).await;
        let service = CollectionService::new(pool.clone());
        let id = service
            .create_smart("Anime", &SmartFilter::default())
            .await
            .expect("create")
            .id;

        let err = service
            .add_members(&id, &["m-1".into()])
            .await
            .expect_err("add on smart");
        assert!(matches!(err, AppError::Validation(_)));
        let err = service
            .remove_member(&id, "m-1")
            .await
            .expect_err("remove on smart");
        assert!(matches!(err, AppError::Validation(_)));
        let err = service
            .reorder(&id, &[])
            .await
            .expect_err("reorder on smart");
        assert!(matches!(err, AppError::Validation(_)));
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn update_smart_rejects_manual_collection_and_unknown_sort() {
        let (pool, path) = migrated_pool("collection_service_smart_rejects.db").await;
        let service = CollectionService::new(pool.clone());
        let manual_id = service.create("Manual").await.expect("create").id;
        let err = service
            .update_smart_filter(&manual_id, &SmartFilter::default())
            .await
            .expect_err("update on manual");
        assert!(matches!(err, AppError::Validation(_)));

        let err = service
            .create_smart(
                "Bad sort",
                &SmartFilter {
                    sort: Some("bogus".into()),
                    ..Default::default()
                },
            )
            .await
            .expect_err("unknown sort");
        assert!(matches!(err, AppError::Validation(_)));
        pool.close().await;
        cleanup_files(&path);
    }
}
