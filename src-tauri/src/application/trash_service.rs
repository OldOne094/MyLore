//! Trash application service (MISSION-044).
//!
//! Soft-delete / restore orchestration over `media` and `trash` repositories.
//! Deleting writes a full JSON before-image of the aggregate to `trash` and
//! then removes the real row (cascades); restoring re-creates the aggregate
//! from that image; purging forgets it forever.

use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::error::AppError;
use crate::infrastructure::repositories::media;
use crate::infrastructure::repositories::trash;

/// A trashed item as surfaced to the UI.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TrashItem {
    pub id: String,
    pub kind: String,
    /// Display title parsed from the before-image payload (media kind).
    pub title: String,
    pub deleted_at: String,
}

/// Trash use-cases.
pub struct TrashService {
    pool: SqlitePool,
}

impl TrashService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Soft-delete a media: store its full before-image in trash, then cascade
    /// the row away. Resolves with the trash id (for undo).
    pub async fn delete_media(&self, id: &str) -> Result<String, AppError> {
        let record = media::get(&self.pool, id)
            .await?
            .ok_or_else(|| AppError::validation(format!("media not found: {id}")))?;

        let payload = serde_json::to_string(&record)?;
        let trash_id = format!("t-{}", Uuid::new_v4());
        let deleted_at = Utc::now().to_rfc3339();

        trash::insert(&self.pool, &trash_id, "media", &payload, &deleted_at).await?;
        media::delete(&self.pool, id).await?;

        Ok(trash_id)
    }

    /// List active trash entries (media + merge kinds) for the trash page.
    pub async fn list_trash(&self) -> Result<Vec<TrashItem>, AppError> {
        let rows = trash::list(&self.pool).await?;
        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            let title = if row.kind == "media" {
                serde_json::from_str::<media::MediaRecord>(&row.payload)
                    .map(|record| record.title_main)
                    .unwrap_or_default()
            } else if row.kind == "merge" {
                serde_json::from_str::<crate::application::merge_service::MergeTrashPayload>(
                    &row.payload,
                )
                .map(|payload| payload.media.title_main)
                .unwrap_or_default()
            } else {
                String::new()
            };
            items.push(TrashItem {
                id: row.id,
                kind: row.kind,
                title,
                deleted_at: row.deleted_at,
            });
        }
        Ok(items)
    }

    /// Restore a soft-deleted aggregate from its trash before-image. For a
    /// plain `media` entry that re-creates the row; for a `merge` entry
    /// (MISSION-089) it reverses the whole merge — re-creates the duplicate,
    /// pulls its nodes back from the survivor, moves a borrowed review /
    /// tracking home and re-adds its collection memberships.
    pub async fn restore_media(&self, id: &str) -> Result<(), AppError> {
        let row = trash::get(&self.pool, id)
            .await?
            .ok_or_else(|| AppError::validation(format!("trash item not found: {id}")))?;
        if row.restored {
            return Err(AppError::validation("this item was already restored"));
        }

        match row.kind.as_str() {
            "media" => self.restore_plain(&row).await?,
            "merge" => self.restore_merge(&row).await?,
            other => {
                return Err(AppError::validation(format!(
                    "cannot restore kind `{other}`"
                )))
            }
        }
        trash::mark_restored(&self.pool, id).await?;
        Ok(())
    }

    async fn restore_plain(
        &self,
        row: &crate::infrastructure::repositories::trash::TrashRow,
    ) -> Result<(), AppError> {
        let record: media::MediaRecord = serde_json::from_str(&row.payload)?;
        media::create(&self.pool, &record).await?;
        Ok(())
    }

    async fn restore_merge(
        &self,
        row: &crate::infrastructure::repositories::trash::TrashRow,
    ) -> Result<(), AppError> {
        let payload: crate::application::merge_service::MergeTrashPayload =
            serde_json::from_str(&row.payload)?;
        let duplicate_id = payload.media.id.clone();

        // Re-create the duplicate exactly as it was before the merge.
        media::create(&self.pool, &payload.media).await?;

        // Pull the merged nodes back — only those still sitting on the
        // survivor (a node deleted since cannot come back).
        for node_id in &payload.node_ids {
            sqlx::query("UPDATE content_node SET media_id = ? WHERE id = ? AND media_id = ?")
                .bind(&duplicate_id)
                .bind(node_id)
                .bind(&payload.survivor_id)
                .execute(&self.pool)
                .await?;
        }

        // A review/tracking that was *moved* (not copied) goes home; the
        // duplicate was just re-created without one, so the move is exact.
        if payload.moved_review {
            sqlx::query("UPDATE review SET media_id = ? WHERE media_id = ?")
                .bind(&duplicate_id)
                .bind(&payload.survivor_id)
                .execute(&self.pool)
                .await?;
        }
        if payload.moved_tracking {
            sqlx::query("UPDATE tracking SET media_id = ? WHERE media_id = ?")
                .bind(&duplicate_id)
                .bind(&payload.survivor_id)
                .execute(&self.pool)
                .await?;
        }

        // Collection memberships are re-added additively: memberships the
        // survivor already had stay untouched.
        for collection_id in &payload.collection_ids {
            sqlx::query(
                "INSERT OR IGNORE INTO collection_member (collection_id, media_id, added_at) \
                 VALUES (?, ?, ?)",
            )
            .bind(collection_id)
            .bind(&duplicate_id)
            .bind(Utc::now().to_rfc3339())
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    /// Permanently forget a trash entry (the aggregate row is already gone).
    pub async fn purge(&self, id: &str) -> Result<(), AppError> {
        trash::purge(&self.pool, id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::media_service::{AddMediaInput, MediaListInput, MediaService};
    use crate::infrastructure::test_support::migrated_pool;

    fn input(title: &str) -> AddMediaInput {
        AddMediaInput {
            title: title.into(),
            content_type: "novel".into(),
            format: Some("light_novel".into()),
            pub_status: Some("ongoing".into()),
            synopsis: Some("A test synopsis.".into()),
            release_year: Some(2026),
            language: Some("ja".into()),
            country: Some("JP".into()),
            pages: Some(320),
            duration_min: None,
            ep_count: None,
            ch_count: None,
            genres: vec!["fantasy".into()],
        }
    }

    #[tokio::test]
    async fn delete_media_moves_to_trash_and_removes() {
        let (pool, _path) = migrated_pool("trash_service_delete.db").await;
        let media_service = MediaService::new(pool.clone());
        let trash_service = TrashService::new(pool.clone());

        let id = media_service
            .add_media(input("Sword of the Dawn"))
            .await
            .expect("add");
        assert!(media_service
            .get_media(id.as_str())
            .await
            .expect("get")
            .is_some());

        let trash_id = trash_service
            .delete_media(id.as_str())
            .await
            .expect("delete");
        assert!(trash_id.starts_with("t-"));

        assert!(media_service
            .get_media(id.as_str())
            .await
            .expect("get")
            .is_none());
        let items = trash_service.list_trash().await.expect("list");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, trash_id);
        assert_eq!(items[0].title, "Sword of the Dawn");
    }

    #[tokio::test]
    async fn delete_media_rejects_unknown_id() {
        let (pool, _path) = migrated_pool("trash_service_delete_missing.db").await;
        let service = TrashService::new(pool.clone());
        let err = service
            .delete_media("m-nope")
            .await
            .expect_err("unknown media rejected");
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[tokio::test]
    async fn restore_brings_the_aggregate_back_and_flag_is_set() {
        let (pool, _path) = migrated_pool("trash_service_restore.db").await;
        let media_service = MediaService::new(pool.clone());
        let trash_service = TrashService::new(pool.clone());

        let id = media_service
            .add_media(input("Red Strings"))
            .await
            .expect("add");
        let trash_id = trash_service
            .delete_media(id.as_str())
            .await
            .expect("delete");

        trash_service
            .restore_media(&trash_id)
            .await
            .expect("restore");

        let restored = media_service.get_media(id.as_str()).await.expect("get");
        let restored = restored.expect("media back");
        assert_eq!(restored.title_main, "Red Strings");
        assert_eq!(restored.genres, vec!["fantasy".to_string()]);
        assert_eq!(restored.release_year, Some(2026));

        let items = trash_service.list_trash().await.expect("list");
        assert!(items.is_empty(), "restored item leaves the active list");

        let err = trash_service
            .restore_media(&trash_id)
            .await
            .expect_err("double restore rejected");
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[tokio::test]
    async fn restore_rejects_unknown_or_non_media() {
        let (pool, _path) = migrated_pool("trash_service_restore_bad.db").await;
        let service = TrashService::new(pool.clone());

        let err = service.restore_media("t-nope").await.expect_err("missing");
        assert!(matches!(err, AppError::Validation(_)));

        // `bulk` is a valid storage kind but has no restore path.
        trash::insert(&pool, "t-x", "bulk", "{}", "2026-01-01T00:00:00Z")
            .await
            .expect("insert");
        let err = service
            .restore_media("t-x")
            .await
            .expect_err("non-media kind");
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[tokio::test]
    async fn purge_forgets_the_entry() {
        let (pool, _path) = migrated_pool("trash_service_purge.db").await;
        let media_service = MediaService::new(pool.clone());
        let trash_service = TrashService::new(pool.clone());

        let id = media_service
            .add_media(input("Gone Tomorrow"))
            .await
            .expect("add");
        let trash_id = trash_service
            .delete_media(id.as_str())
            .await
            .expect("delete");

        trash_service.purge(&trash_id).await.expect("purge");
        assert!(trash_service.list_trash().await.expect("list").is_empty());
        assert!(media_service
            .get_media(id.as_str())
            .await
            .expect("get")
            .is_none());
    }

    #[tokio::test]
    async fn delete_invalidates_library_listing() {
        let (pool, _path) = migrated_pool("trash_service_list_invalidate.db").await;
        let media_service = MediaService::new(pool.clone());
        let trash_service = TrashService::new(pool.clone());

        media_service
            .add_media(input("One"))
            .await
            .expect("add one");
        let two = media_service
            .add_media(input("Two"))
            .await
            .expect("add two");

        trash_service
            .delete_media(two.as_str())
            .await
            .expect("delete two");

        let items = media_service
            .list_media(MediaListInput::default())
            .await
            .expect("list");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "One");
    }
}
