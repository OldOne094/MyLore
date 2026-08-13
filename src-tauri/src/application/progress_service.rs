//! Per-node progress service (MISSION-047).
//!
//! Use-cases for marking nodes read/watched/skipped/unread, including range
//! marks. Writes go through the tracking repository; a completed state
//! (`read`/`watched`) always carries a `read_at` timestamp (DOMAIN_MODEL §2.7)
//! minted here — repositories stay clock-free.

use std::str::FromStr;

use chrono::Utc;
use sqlx::SqlitePool;

use crate::application::node_service::{ContentNode, NodeService};
use crate::application::tracking_service::TrackingService;
use crate::domain::enums::{ContentType, NodeProgressState};
use crate::domain::progress::ProgressTemplate;
use crate::error::AppError;
use crate::infrastructure::repositories::media;
use crate::infrastructure::repositories::node;
use crate::infrastructure::repositories::tracking::{self, MediaProgressSummary, NodeProgress};

/// A library row's progress summary (MISSION-049): weighted totals (pages for
/// books, else unit counts) + the next not-yet-consumed unit. `next_label` is a
/// short human label ("E4", "Ch7") for the in-grid quick-control button.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProgressSummary {
    pub percent: Option<u8>,
    pub completed: i64,
    pub total: i64,
    pub next_label: Option<String>,
    pub next_node_id: Option<String>,
}

/// The result of a `node_progress_next` write.
#[derive(Debug, Clone, serde::Serialize)]
pub struct NodeProgressNextView {
    pub media_id: String,
    pub summary: ProgressSummary,
}

/// Per-node progress use-cases.
pub struct ProgressService {
    pool: SqlitePool,
}

impl ProgressService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Set the progress state of a single node. Rejects unknown node ids and
    /// invalid states. Runs the auto-status rule afterwards (MISSION-048).
    pub async fn set_node_progress(&self, node_id: &str, state: &str) -> Result<(), AppError> {
        let state = NodeProgressState::from_str(state).map_err(|err| {
            crate::error::AppError::validation(format!("invalid node progress state: {err}"))
        })?;
        let node = node::get(&self.pool, node_id).await?;
        let Some(node) = node else {
            return Err(AppError::validation(format!("node not found: {node_id}")));
        };
        tracking::set_progress(&self.pool, &self.progress_row(node_id, state).await).await?;
        self.sync_auto_status(&node.media_id).await;
        Ok(())
    }

    /// Set the same progress state for every node between `from_id` and
    /// `to_id` in the media's display order (tree preorder), in one
    /// transaction. Resolves with the node ids updated, in display order.
    /// Rejects when either bound is not a node of the media.
    pub async fn set_range_progress(
        &self,
        media_id: &str,
        from_id: &str,
        to_id: &str,
        state: &str,
    ) -> Result<Vec<String>, AppError> {
        let state = NodeProgressState::from_str(state).map_err(|err| {
            crate::error::AppError::validation(format!("invalid node progress state: {err}"))
        })?;
        let tree = NodeService::new(self.pool.clone())
            .tree_for_media(media_id)
            .await?;
        let order = preorder(&tree);
        let from_index = order
            .iter()
            .position(|id| id == from_id)
            .ok_or_else(|| AppError::validation(format!("node not in media: {from_id}")))?;
        let to_index = order
            .iter()
            .position(|id| id == to_id)
            .ok_or_else(|| AppError::validation(format!("node not in media: {to_id}")))?;
        let (start, end) = if from_index <= to_index {
            (from_index, to_index)
        } else {
            (to_index, from_index)
        };

        let mut rows = Vec::new();
        for id in &order[start..=end] {
            rows.push(self.progress_row(id, state).await);
        }
        let written = tracking::set_progress_many(&self.pool, &rows).await?;
        self.sync_auto_status(media_id).await;
        Ok(written)
    }

    /// Run the auto-status rule after a progress write. The write itself has
    /// already committed; a sync failure must not surface as a progress failure
    /// (the UI would roll back a persisted change), so it is logged only.
    async fn sync_auto_status(&self, media_id: &str) {
        let service = TrackingService::new(self.pool.clone());
        if let Err(err) = service.sync_auto_status(media_id).await {
            tracing::warn!(%err, media_id, "auto status sync failed after progress write");
        }
    }

    /// Mark the next not-yet-consumed countable node of a media done (watched
    /// for episode units, read otherwise), then run the auto-status rule.
    /// Resolves with the refreshed progress summary, or `None` when there is
    /// nothing left to mark. Rejects for unknown media.
    pub async fn mark_next_unit(
        &self,
        media_id: &str,
    ) -> Result<Option<NodeProgressNextView>, AppError> {
        let Some(media_row) = media::get(&self.pool, media_id).await? else {
            return Err(AppError::validation(format!("media not found: {media_id}")));
        };
        let content_type = ContentType::from_str(&media_row.content_type)?;
        let template = ProgressTemplate::for_content_type(content_type);
        let Some(unit) = tracking::next_unread_unit(&self.pool, media_id).await? else {
            return Ok(None);
        };

        let now = Utc::now().to_rfc3339();
        tracking::set_progress(
            &self.pool,
            &NodeProgress {
                node_id: unit.node_id.clone(),
                state: template.consuming_state.as_str().to_string(),
                read_at: Some(now.clone()),
                note: None,
                rating: None,
                updated_at: now,
            },
        )
        .await?;
        self.sync_auto_status(media_id).await;
        let summary = self.summary_for(media_id).await?;
        Ok(Some(NodeProgressNextView {
            media_id: media_id.to_string(),
            summary,
        }))
    }

    /// Read a media's progress summary (empty defaults when the media has no
    /// countable nodes).
    pub async fn summary_for(&self, media_id: &str) -> Result<ProgressSummary, AppError> {
        let summaries = tracking::progress_summaries(&self.pool, &[media_id.to_string()]).await?;
        Ok(
            match summaries.into_iter().find(|s| s.media_id == media_id) {
                Some(row) => summary_dto(&row),
                None => ProgressSummary {
                    percent: None,
                    completed: 0,
                    total: 0,
                    next_label: None,
                    next_node_id: None,
                },
            },
        )
    }

    /// Build a progress row for one node. Completed states stamp `read_at`;
    /// all other states clear it.
    async fn progress_row(&self, node_id: &str, state: NodeProgressState) -> NodeProgress {
        let now = Utc::now().to_rfc3339();
        let read_at = state.is_completed().then(|| now.clone());
        NodeProgress {
            node_id: node_id.to_string(),
            state: state.as_str().to_string(),
            read_at,
            note: None,
            rating: None,
            updated_at: now,
        }
    }
}

/// The media's node ids in display order (tree preorder, matching the UI).
fn preorder_ids(nodes: &[ContentNode], out: &mut Vec<String>) {
    for node in nodes {
        out.push(node.id.clone());
        preorder_ids(&node.children, out);
    }
}

fn preorder(nodes: &[ContentNode]) -> Vec<String> {
    let mut ids = Vec::new();
    preorder_ids(nodes, &mut ids);
    ids
}

/// Map a repo progress summary onto the serializable DTO (percent + labels).
pub(crate) fn summary_dto(row: &MediaProgressSummary) -> ProgressSummary {
    let percent = (row.total_weight > 0)
        .then(|| (row.completed_weight.saturating_mul(100) / row.total_weight) as u8);
    ProgressSummary {
        percent,
        completed: row.completed_weight,
        total: row.total_weight,
        next_label: row.next_node_id.as_ref().map(|_| {
            unit_label(
                row.next_kind.as_deref().unwrap_or("node"),
                row.next_number.as_deref(),
                row.next_position,
            )
        }),
        next_node_id: row.next_node_id.clone(),
    }
}

/// A short label for a countable node: "E4" for episodes, "Ch7" for chapters,
/// "#12" otherwise (falls back to the display position when unnumbered).
pub(crate) fn unit_label(kind: &str, number: Option<&str>, position: Option<i64>) -> String {
    let raw = match number {
        Some(n) if !n.trim().is_empty() => n.to_string(),
        _ => position.map(|p| p.to_string()).unwrap_or_default(),
    };
    match kind {
        "episode" => format!("E{raw}"),
        "chapter" => format!("Ch{raw}"),
        _ if raw.is_empty() => "#".to_string(),
        _ => format!("#{raw}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

    async fn seed_media(pool: &sqlx::SqlitePool, id: &str) {
        media::create(
            pool,
            &media::MediaRecord {
                id: id.to_string(),
                content_type: "manga".into(),
                format: None,
                title_main: format!("Title {id}"),
                title_original: None,
                synopsis: None,
                pub_status: "unknown".into(),
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
    }

    /// manga tree: v1 (c1, c2), v2 (c3)
    async fn seed_tree(pool: &sqlx::SqlitePool) {
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
    async fn set_node_progress_stamps_read_at_for_completed() {
        let (pool, path) = migrated_pool("progress_service_set.db").await;
        seed_media(&pool, "m-1").await;
        seed_tree(&pool).await;

        let service = ProgressService::new(pool.clone());
        service
            .set_node_progress("c1", "read")
            .await
            .expect("mark read");

        let got = tracking::get_progress(&pool, "c1")
            .await
            .expect("get")
            .expect("stored");
        assert_eq!(got.state, "read");
        assert!(got.read_at.is_some(), "read implies read_at");

        service
            .set_node_progress("c1", "unread")
            .await
            .expect("mark unread");
        let got = tracking::get_progress(&pool, "c1")
            .await
            .expect("get")
            .expect("stored");
        assert_eq!(got.state, "unread");
        assert!(got.read_at.is_none(), "unread clears read_at");

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn set_node_progress_rejects_unknown_node_and_state() {
        let (pool, path) = migrated_pool("progress_service_invalid.db").await;
        seed_media(&pool, "m-1").await;
        seed_tree(&pool).await;
        let service = ProgressService::new(pool.clone());

        let err = service
            .set_node_progress("nope", "read")
            .await
            .expect_err("bad node");
        assert!(matches!(err, AppError::Validation(_)));

        let err = service
            .set_node_progress("c1", "finished")
            .await
            .expect_err("bad state");
        assert!(matches!(err, AppError::Validation(_)));

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn range_marks_forward_and_reverse_order() {
        let (pool, path) = migrated_pool("progress_service_range.db").await;
        seed_media(&pool, "m-1").await;
        seed_tree(&pool).await;
        let service = ProgressService::new(pool.clone());
        let (_v1, v2, c1, c2, c3) = ("v1", "v2", "c1", "c2", "c3");

        let written = service
            .set_range_progress("m-1", c1, c3, "read")
            .await
            .expect("range c1..c3");
        assert_eq!(written, vec![c1, c2, v2, c3]);

        let written = service
            .set_range_progress("m-1", c3, c1, "skipped")
            .await
            .expect("reversed range");
        assert_eq!(written, vec![c1, c2, v2, c3]);
        for id in [c1, c2, v2, c3] {
            let got = tracking::get_progress(&pool, id)
                .await
                .expect("get")
                .unwrap();
            assert_eq!(got.state, "skipped");
        }

        let aggregate = NodeService::new(pool.clone())
            .tree_for_media("m-1")
            .await
            .expect("tree");
        assert_eq!(
            aggregate[0].state.as_deref(),
            None,
            "v1 was outside the range and untouched"
        );
        assert_eq!(aggregate[0].children[1].state.as_deref(), Some("skipped"));

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn range_marks_single_node_when_bounds_are_equal() {
        let (pool, path) = migrated_pool("progress_service_range_single.db").await;
        seed_media(&pool, "m-1").await;
        seed_tree(&pool).await;
        let service = ProgressService::new(pool.clone());

        let written = service
            .set_range_progress("m-1", "c2", "c2", "read")
            .await
            .expect("single range");
        assert_eq!(written, vec!["c2"]);

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn range_rejects_bounds_not_in_the_media() {
        let (pool, path) = migrated_pool("progress_service_range_bad.db").await;
        seed_media(&pool, "m-1").await;
        seed_tree(&pool).await;
        let service = ProgressService::new(pool.clone());

        let err = service
            .set_range_progress("m-1", "missing", "c1", "read")
            .await
            .expect_err("missing bound");
        assert!(matches!(err, AppError::Validation(_)));

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn preorder_matches_display_order() {
        let nodes = [
            ContentNode {
                id: "v1".into(),
                kind: "volume".into(),
                position: 1,
                number: None,
                title: None,
                release_date: None,
                duration_min: None,
                page_count: None,
                synopsis: None,
                is_special: false,
                state: None,
                children: vec![
                    ContentNode {
                        id: "c1".into(),
                        kind: "chapter".into(),
                        position: 1,
                        number: None,
                        title: None,
                        release_date: None,
                        duration_min: None,
                        page_count: None,
                        synopsis: None,
                        is_special: false,
                        state: None,
                        children: vec![],
                    },
                    ContentNode {
                        id: "c2".into(),
                        kind: "chapter".into(),
                        position: 2,
                        number: None,
                        title: None,
                        release_date: None,
                        duration_min: None,
                        page_count: None,
                        synopsis: None,
                        is_special: false,
                        state: None,
                        children: vec![],
                    },
                ],
            },
            ContentNode {
                id: "v2".into(),
                kind: "volume".into(),
                position: 2,
                number: None,
                title: None,
                release_date: None,
                duration_min: None,
                page_count: None,
                synopsis: None,
                is_special: false,
                state: None,
                children: vec![],
            },
        ];
        assert_eq!(preorder(&nodes), vec!["v1", "c1", "c2", "v2"]);
    }

    #[tokio::test]
    async fn mark_next_unit_watches_episodes_and_auto_completes() {
        let (pool, path) = migrated_pool("progress_service_next_anime.db").await;
        let media_id = seed_anime(&pool).await;
        seed_episode(&pool, "e1", &media_id, "1").await;
        seed_episode(&pool, "e2", &media_id, "2").await;
        seed_episode(&pool, "e3", &media_id, "3").await;
        let service = ProgressService::new(pool.clone());

        let first = service
            .mark_next_unit(&media_id)
            .await
            .expect("mark")
            .expect("has next");
        assert_eq!(first.media_id, media_id);
        assert_eq!(first.summary.completed, 1);
        assert_eq!(first.summary.next_label.as_deref(), Some("E2"));
        assert_eq!(
            tracking::get_progress(&pool, "e1")
                .await
                .expect("get")
                .unwrap()
                .state,
            "watched"
        );

        service
            .mark_next_unit(&media_id)
            .await
            .expect("mark")
            .expect("has next");
        let done = service
            .mark_next_unit(&media_id)
            .await
            .expect("mark")
            .expect("still a summary");
        assert!(done.summary.next_node_id.is_none(), "everything watched");

        let exhausted = service.mark_next_unit(&media_id).await.expect("mark");
        assert!(exhausted.is_none(), "nothing left to mark");
        let tracking_row = tracking::get_tracking(&pool, &media_id)
            .await
            .expect("tracking")
            .expect("row");
        assert_eq!(tracking_row.core_status, "completed", "auto-complete ran");
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn mark_next_unit_skips_to_next_and_rejects_unknown_media() {
        let (pool, path) = migrated_pool("progress_service_next_skip.db").await;
        let media_id = seed_anime(&pool).await;
        seed_episode(&pool, "e1", &media_id, "1").await;
        seed_episode(&pool, "e2", &media_id, "2").await;
        let service = ProgressService::new(pool.clone());

        service
            .set_node_progress("e1", "skipped")
            .await
            .expect("skip e1");
        let next = service
            .mark_next_unit(&media_id)
            .await
            .expect("mark")
            .expect("has next");
        assert_eq!(
            next.summary.next_label.as_deref(),
            Some("E2"),
            "skipped e1 is still the next unit"
        );

        let err = service
            .mark_next_unit("m-nope")
            .await
            .expect_err("unknown media");
        assert!(matches!(err, AppError::Validation(_)));
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn mark_next_unit_reads_books_and_weights_pages() {
        let (pool, path) = migrated_pool("progress_service_next_book.db").await;
        let media_id = seed_book(&pool).await;
        seed_chapter(&pool, "c1", &media_id, 30).await;
        seed_chapter(&pool, "c2", &media_id, 45).await;
        let service = ProgressService::new(pool.clone());

        let next = service
            .mark_next_unit(&media_id)
            .await
            .expect("mark")
            .expect("has next");
        assert_eq!(next.summary.completed, 30, "c1's page weight");
        assert_eq!(next.summary.total, 75);
        assert_eq!(next.summary.percent, Some(40));
        assert_eq!(next.summary.next_label.as_deref(), Some("Ch2"));
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn unit_label_formats_kinds_and_fallbacks() {
        assert_eq!(unit_label("episode", Some("12.5"), Some(12)), "E12.5");
        assert_eq!(unit_label("chapter", Some("7"), Some(7)), "Ch7");
        assert_eq!(unit_label("episode", None, Some(3)), "E3");
        assert_eq!(unit_label("node", None, Some(2)), "#2");
        assert_eq!(unit_label("chapter", Some(" "), Some(9)), "Ch9");
    }

    /// Seed an anime media (counts watched episodes).
    async fn seed_anime(pool: &sqlx::SqlitePool) -> String {
        let id = format!("m-{}", uuid::Uuid::new_v4());
        seed_media_row(pool, &id, "anime").await;
        id
    }

    /// Seed a book media (weighs chapters by pages).
    async fn seed_book(pool: &sqlx::SqlitePool) -> String {
        let id = format!("m-{}", uuid::Uuid::new_v4());
        seed_media_row(pool, &id, "book").await;
        id
    }

    async fn seed_media_row(pool: &sqlx::SqlitePool, id: &str, content_type: &str) {
        media::create(
            pool,
            &media::MediaRecord {
                id: id.to_string(),
                content_type: content_type.into(),
                format: None,
                title_main: format!("Title {id}"),
                title_original: None,
                synopsis: None,
                pub_status: "unknown".into(),
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
    }

    async fn seed_episode(pool: &sqlx::SqlitePool, id: &str, media_id: &str, number: &str) {
        node::create(
            pool,
            &node::NodeRecord {
                id: id.to_string(),
                media_id: media_id.to_string(),
                parent_id: None,
                kind: "episode".into(),
                position: number.parse().expect("position"),
                number: Some(number.to_string()),
                title: None,
                release_date: None,
                duration_min: Some(24),
                page_count: None,
                synopsis: None,
                external_id: None,
                is_special: false,
                created_at: "2026-01-01".into(),
            },
        )
        .await
        .expect("seed episode");
    }

    async fn seed_chapter(pool: &sqlx::SqlitePool, id: &str, media_id: &str, pages: i64) {
        node::create(
            pool,
            &node::NodeRecord {
                id: id.to_string(),
                media_id: media_id.to_string(),
                parent_id: None,
                kind: "chapter".into(),
                position: pages,
                number: Some(id.trim_start_matches('c').to_string()),
                title: None,
                release_date: None,
                duration_min: None,
                page_count: Some(pages),
                synopsis: None,
                external_id: None,
                is_special: false,
                created_at: "2026-01-01".into(),
            },
        )
        .await
        .expect("seed chapter");
    }
}
