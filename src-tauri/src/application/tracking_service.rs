//! Tracking service (MISSION-048).
//!
//! Single-media status use-cases on top of the domain status engine
//! (MISSION-024): read the tracking row, apply a status transition, and run
//! the **auto-complete rule** — after every progress write the status implied
//! by the aggregate (`suggest_auto_status`) is applied through the engine when
//! it differs from the current one. The rule owns the consumption lifecycle
//! (planned → in progress → completed) and is *reversible*: un-marking the
//! last consumed node moves a completed title back to in progress → planned.
//! Manual statuses (on_hold, dropped, wishlist, repeat) are never
//! auto-overridden; per-media auto-track toggling arrives with MISSION-052.
//!
//! The record ↔ domain mapping helpers live here so `bulk_service` and this
//! service share one path into the engine.

use std::str::FromStr;

use chrono::Utc;
use sqlx::SqlitePool;

use crate::domain::enums::{ContentType, CoreStatus, NodeKind, NodeProgressState};
use crate::domain::progress::{aggregate, NodeTick};
use crate::domain::status::{apply_transition, suggest_auto_status};
use crate::domain::tracking::Tracking;
use crate::domain::value_objects::{DateOnly, MediaId};
use crate::error::AppError;
use crate::infrastructure::repositories::{media, tracking};

/// The tracking row surfaced to the detail page (status picker + repeat
/// counter + dates).
#[derive(Debug, Clone, serde::Serialize)]
pub struct TrackingView {
    pub media_id: String,
    pub core_status: String,
    pub custom_status_id: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub repeat_count: u32,
    pub updated_at: String,
}

/// Tracking use-cases for a single media.
pub struct TrackingService {
    pool: SqlitePool,
}

impl TrackingService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Read the tracking row for a media (`None` when untracked).
    pub async fn get(&self, media_id: &str) -> Result<Option<TrackingView>, AppError> {
        let row = tracking::get_tracking(&self.pool, media_id).await?;
        Ok(row.as_ref().map(view_from_record))
    }

    /// Apply a status transition for one media through the status engine
    /// (stamps started_at / finished_at, maintains the repeat counter).
    /// Rejects unknown targets, unknown media, and Repeat without prior
    /// consumption. Resolves with the resulting tracking row.
    pub async fn set_status(
        &self,
        media_id: &str,
        core_status: &str,
    ) -> Result<TrackingView, AppError> {
        let media_id = MediaId::new(media_id)?;
        if media::get(&self.pool, media_id.as_str()).await?.is_none() {
            return Err(AppError::validation(format!(
                "media not found: {}",
                media_id.as_str()
            )));
        }
        let to = CoreStatus::from_str(core_status)?;
        let today = DateOnly::new(Utc::now().format("%Y-%m-%d").to_string())?;
        let updated_at = Utc::now().to_rfc3339();
        let existing = tracking::get_tracking(&self.pool, media_id.as_str()).await?;
        let current = existing_to_domain(existing, &media_id, &updated_at)?;
        let next = apply_transition(&current, to, &today)
            .map_err(|err| AppError::validation(format!("{}: {}", media_id.as_str(), err)))?;
        let mut next = next;
        next.updated_at = updated_at;
        tracking::upsert_tracking(&self.pool, &domain_to_record(&next)).await?;
        Ok(view_from_domain(&next))
    }

    /// The auto-complete rule. Derives the status implied by the media's
    /// progress aggregate and applies it when it differs from the current one —
    /// only for statuses the engine owns (planned / in_progress / completed).
    /// Resolves with the updated view when a transition happened, else `None`.
    pub async fn sync_auto_status(&self, media_id: &str) -> Result<Option<TrackingView>, AppError> {
        let media = media::get(&self.pool, media_id).await?;
        let Some(media) = media else {
            return Err(AppError::validation(format!("media not found: {media_id}")));
        };
        let content_type = ContentType::from_str(&media.content_type)?;
        let ticks = ticks_to_domain(&tracking::node_ticks_for_media(&self.pool, media_id).await?)?;
        let Some(suggestion) = suggest_auto_status(&aggregate(content_type, &ticks)) else {
            return Ok(None); // no node data → nothing to reason about
        };

        let media_id = MediaId::new(media_id)?;
        let updated_at = Utc::now().to_rfc3339();
        let existing = tracking::get_tracking(&self.pool, media_id.as_str()).await?;
        let current = existing_to_domain(existing, &media_id, &updated_at)?;
        if current.core_status == suggestion {
            return Ok(None);
        }
        if !matches!(
            current.core_status,
            CoreStatus::Planned | CoreStatus::InProgress | CoreStatus::Completed
        ) {
            return Ok(None); // manual statuses are never auto-overridden
        }

        let today = DateOnly::new(Utc::now().format("%Y-%m-%d").to_string())?;
        let next = apply_transition(&current, suggestion, &today)
            .map_err(|err| AppError::validation(format!("{}: {}", media_id.as_str(), err)))?;
        let mut next = next;
        next.updated_at = updated_at;
        tracking::upsert_tracking(&self.pool, &domain_to_record(&next)).await?;
        Ok(Some(view_from_domain(&next)))
    }
}

/// Map repo tick rows into the domain ticks the progress engine folds. Node
/// kinds and states are CHECK-constrained in the schema, so a parse failure is
/// a real data bug surfaced as a validation error.
fn ticks_to_domain(rows: &[tracking::NodeTickRow]) -> Result<Vec<NodeTick>, AppError> {
    rows.iter()
        .map(|row| {
            let kind = NodeKind::from_str(&row.kind)
                .map_err(|err| AppError::validation(format!("invalid node kind: {err}")))?;
            let state = NodeProgressState::from_str(&row.state)
                .map_err(|err| AppError::validation(format!("invalid node state: {err}")))?;
            Ok(NodeTick {
                id: row.node_id.clone(),
                kind,
                state,
                page_count: row
                    .page_count
                    .map(|p| {
                        u32::try_from(p)
                            .map_err(|_| AppError::validation("page count out of range"))
                    })
                    .transpose()?,
                duration_min: row
                    .duration_min
                    .map(|d| {
                        u32::try_from(d).map_err(|_| AppError::validation("duration out of range"))
                    })
                    .transpose()?,
            })
        })
        .collect()
}

fn view_from_domain(tracking: &Tracking) -> TrackingView {
    TrackingView {
        media_id: tracking.media_id.as_str().to_string(),
        core_status: tracking.core_status.as_str().to_string(),
        custom_status_id: tracking.custom_status_id.clone(),
        started_at: tracking.started_at.as_ref().map(|d| d.as_str().to_string()),
        finished_at: tracking
            .finished_at
            .as_ref()
            .map(|d| d.as_str().to_string()),
        repeat_count: tracking.repeat_count,
        updated_at: tracking.updated_at.clone(),
    }
}

fn view_from_record(record: &tracking::TrackingRecord) -> TrackingView {
    TrackingView {
        media_id: record.media_id.clone(),
        core_status: record.core_status.clone(),
        custom_status_id: record.custom_status_id.clone(),
        started_at: record.started_at.clone(),
        finished_at: record.finished_at.clone(),
        repeat_count: u32::try_from(record.repeat_count).unwrap_or(0),
        updated_at: record.updated_at.clone(),
    }
}

/// Map a persisted tracking row (or nothing) into the domain aggregate. A
/// missing row becomes a fresh `Planned` record; the status engine then moves
/// it to the requested status.
pub(crate) fn existing_to_domain(
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
pub(crate) fn domain_to_record(next: &Tracking) -> tracking::TrackingRecord {
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
    use crate::application::progress_service::ProgressService;
    use crate::infrastructure::repositories::media;
    use crate::infrastructure::repositories::node;
    use crate::infrastructure::test_support::{cleanup_files, migrated_pool};

    fn sample_node(
        id: &str,
        media_id: &str,
        parent_id: Option<&str>,
        kind: &str,
        position: i64,
    ) -> node::NodeRecord {
        node::NodeRecord {
            id: id.to_string(),
            media_id: media_id.to_string(),
            parent_id: parent_id.map(str::to_string),
            kind: kind.to_string(),
            position,
            number: None,
            title: None,
            release_date: None,
            duration_min: None,
            page_count: None,
            synopsis: None,
            external_id: None,
            is_special: false,
            created_at: "2026-01-01".to_string(),
        }
    }

    async fn seed_media(pool: &SqlitePool) -> String {
        media::create(
            pool,
            &media::MediaRecord {
                id: "m-1".into(),
                content_type: "manga".into(),
                format: None,
                title_main: "Title".into(),
                title_original: None,
                synopsis: None,
                pub_status: "ongoing".into(),
                start_date: None,
                end_date: None,
                release_year: None,
                language: None,
                country: None,
                content_rating: None,
                pages: None,
                duration_min: None,
                ep_count: None,
                ch_count: None,
                cover_asset_id: None,
                banner_asset_id: None,
                provider: None,
                provider_url: None,
                metadata_refreshed_at: None,
                created_at: "2026-01-01".into(),
                updated_at: "2026-01-01".into(),
                alt_titles: Vec::new(),
                people: Vec::new(),
                genres: Vec::new(),
                tags: Vec::new(),
                external_ids: Vec::new(),
                relations: Vec::new(),
            },
        )
        .await
        .expect("seed media");
        "m-1".into()
    }

    /// manga tree: v1 (c1, c2), v2 (c3) — three countable chapters.
    async fn seed_tree(pool: &SqlitePool) {
        node::create(pool, &sample_node("v1", "m-1", None, "volume", 1))
            .await
            .expect("create v1");
        node::create(pool, &sample_node("c1", "m-1", Some("v1"), "chapter", 1))
            .await
            .expect("create c1");
        node::create(pool, &sample_node("c2", "m-1", Some("v1"), "chapter", 2))
            .await
            .expect("create c2");
        node::create(pool, &sample_node("v2", "m-1", None, "volume", 2))
            .await
            .expect("create v2");
        node::create(pool, &sample_node("c3", "m-1", Some("v2"), "chapter", 1))
            .await
            .expect("create c3");
    }

    #[tokio::test]
    async fn set_status_creates_row_and_stamps_dates() {
        let (pool, path) = migrated_pool("tracking_service_create.db").await;
        seed_media(&pool).await;
        let service = TrackingService::new(pool.clone());

        let view = service
            .set_status("m-1", "in_progress")
            .await
            .expect("start");
        assert_eq!(view.core_status, "in_progress");
        assert!(view.started_at.is_some(), "started_at stamped");
        assert!(view.finished_at.is_none());

        let view = service
            .set_status("m-1", "completed")
            .await
            .expect("complete");
        assert_eq!(view.core_status, "completed");
        assert!(view.finished_at.is_some(), "finished_at stamped");
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn set_status_repeats_and_resets_repeat_count() {
        let (pool, path) = migrated_pool("tracking_service_repeat.db").await;
        seed_media(&pool).await;
        let service = TrackingService::new(pool.clone());

        service
            .set_status("m-1", "completed")
            .await
            .expect("complete");
        let repeat = service.set_status("m-1", "repeat").await.expect("repeat");
        assert_eq!(repeat.core_status, "repeat");
        assert_eq!(repeat.repeat_count, 1);

        let again = service
            .set_status("m-1", "repeat")
            .await
            .expect("repeat again");
        assert_eq!(again.repeat_count, 2);

        let held = service.set_status("m-1", "on_hold").await.expect("on hold");
        assert_eq!(held.repeat_count, 0, "leaving repeat resets the counter");
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn set_status_rejects_repeat_without_consumption() {
        let (pool, path) = migrated_pool("tracking_service_repeat_guard.db").await;
        seed_media(&pool).await;
        let service = TrackingService::new(pool.clone());

        let err = service
            .set_status("m-1", "repeat")
            .await
            .expect_err("repeat guard");
        assert!(matches!(err, AppError::Validation(_)));
        assert!(
            tracking::get_tracking(&pool, "m-1")
                .await
                .expect("get")
                .is_none(),
            "failed transition writes nothing"
        );
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn set_status_rejects_unknown_status_and_media() {
        let (pool, path) = migrated_pool("tracking_service_bad.db").await;
        seed_media(&pool).await;
        let service = TrackingService::new(pool.clone());

        let err = service
            .set_status("m-1", "watching")
            .await
            .expect_err("unknown status");
        assert!(matches!(err, AppError::Validation(_)));

        let err = service
            .set_status("nope", "planned")
            .await
            .expect_err("unknown media");
        assert!(matches!(err, AppError::Validation(_)));
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn get_returns_none_for_untracked_and_view_after_write() {
        let (pool, path) = migrated_pool("tracking_service_get.db").await;
        seed_media(&pool).await;
        let service = TrackingService::new(pool.clone());

        assert!(service.get("m-1").await.expect("get").is_none());
        service.set_status("m-1", "completed").await.expect("set");
        let view = service.get("m-1").await.expect("get").unwrap();
        assert_eq!(view.core_status, "completed");
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn auto_completes_when_all_nodes_consumed() {
        let (pool, path) = migrated_pool("tracking_auto_complete.db").await;
        seed_media(&pool).await;
        seed_tree(&pool).await;
        let progress = ProgressService::new(pool.clone());
        let tracking_service = TrackingService::new(pool.clone());

        progress
            .set_node_progress("c1", "read")
            .await
            .expect("mark c1");
        let view = tracking_service.get("m-1").await.expect("get").unwrap();
        assert_eq!(view.core_status, "in_progress", "partial → in progress");

        progress
            .set_node_progress("c2", "read")
            .await
            .expect("mark c2");
        progress
            .set_node_progress("c3", "read")
            .await
            .expect("mark c3");
        let view = tracking_service.get("m-1").await.expect("get").unwrap();
        assert_eq!(view.core_status, "completed", "all chapters → completed");
        assert!(view.finished_at.is_some(), "completed stamps finished_at");
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn auto_status_is_reversible_when_unmarking() {
        let (pool, path) = migrated_pool("tracking_auto_reversible.db").await;
        seed_media(&pool).await;
        seed_tree(&pool).await;
        let progress = ProgressService::new(pool.clone());
        let tracking_service = TrackingService::new(pool.clone());

        for id in ["c1", "c2", "c3"] {
            progress.set_node_progress(id, "read").await.expect("mark");
        }
        assert_eq!(
            tracking_service
                .get("m-1")
                .await
                .expect("get")
                .unwrap()
                .core_status,
            "completed"
        );

        progress
            .set_node_progress("c3", "unread")
            .await
            .expect("unmark last");
        let view = tracking_service.get("m-1").await.expect("get").unwrap();
        assert_eq!(
            view.core_status, "in_progress",
            "reversible: back to in progress"
        );
        assert!(view.finished_at.is_none(), "finish cleared on reversal");

        progress
            .set_node_progress("c2", "unread")
            .await
            .expect("unmark again");
        progress
            .set_node_progress("c1", "unread")
            .await
            .expect("unmark all");
        let view = tracking_service.get("m-1").await.expect("get").unwrap();
        assert_eq!(view.core_status, "planned", "nothing consumed → planned");
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn auto_never_overrides_manual_statuses() {
        let (pool, path) = migrated_pool("tracking_auto_manual.db").await;
        seed_media(&pool).await;
        seed_tree(&pool).await;
        let progress = ProgressService::new(pool.clone());
        let tracking_service = TrackingService::new(pool.clone());

        tracking_service
            .set_status("m-1", "on_hold")
            .await
            .expect("manual on_hold");
        progress
            .set_node_progress("c1", "read")
            .await
            .expect("mark");
        let view = tracking_service.get("m-1").await.expect("get").unwrap();
        assert_eq!(
            view.core_status, "on_hold",
            "on_hold is never auto-overridden"
        );

        tracking_service
            .set_status("m-1", "completed")
            .await
            .expect("manual completed");
        tracking_service
            .set_status("m-1", "repeat")
            .await
            .expect("manual repeat");
        for id in ["c1", "c2", "c3"] {
            progress.set_node_progress(id, "read").await.expect("mark");
        }
        let view = tracking_service.get("m-1").await.expect("get").unwrap();
        assert_eq!(
            view.core_status, "repeat",
            "repeat run keeps its status even when everything is consumed"
        );
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn auto_creates_no_tracking_without_node_data() {
        let (pool, path) = migrated_pool("tracking_auto_empty.db").await;
        seed_media(&pool).await;
        let tracking_service = TrackingService::new(pool.clone());

        // No nodes → the aggregate has no units → no suggestion → no row.
        assert!(tracking_service.get("m-1").await.expect("get").is_none());
        let synced = tracking_service
            .sync_auto_status("m-1")
            .await
            .expect("sync");
        assert!(synced.is_none());
        assert!(tracking::get_tracking(&pool, "m-1")
            .await
            .expect("get")
            .is_none());
        pool.close().await;
        cleanup_files(&path);
    }
}
