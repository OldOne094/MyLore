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
use crate::domain::enums::NodeProgressState;
use crate::error::AppError;
use crate::infrastructure::repositories::node;
use crate::infrastructure::repositories::tracking::{self, NodeProgress};

/// Per-node progress use-cases.
pub struct ProgressService {
    pool: SqlitePool,
}

impl ProgressService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Set the progress state of a single node. Rejects unknown node ids and
    /// invalid states.
    pub async fn set_node_progress(&self, node_id: &str, state: &str) -> Result<(), AppError> {
        let state = NodeProgressState::from_str(state).map_err(|err| {
            crate::error::AppError::validation(format!("invalid node progress state: {err}"))
        })?;
        if node::get(&self.pool, node_id).await?.is_none() {
            return Err(AppError::validation(format!("node not found: {node_id}")));
        }
        tracking::set_progress(&self.pool, &self.progress_row(node_id, state).await).await
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
        tracking::set_progress_many(&self.pool, &rows).await
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
}
