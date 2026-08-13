//! Content-node tree service (MISSION-046).
//!
//! Reads a media's node rows and assembles the nested tree the detail page
//! renders with expand/collapse (seasons→episodes, volumes→chapters). Reads
//! only; per-node progress commands land with MISSION-047.

use std::collections::HashMap;

use sqlx::SqlitePool;

use crate::error::AppError;
use crate::infrastructure::repositories::node::{list_by_media, NodeRecord};

/// A node in the serializable tree returned to the UI. Mirrors the `ContentNode`
/// interface in the IPC contract (`scripts/ipc-contract.json`).
#[derive(Debug, Clone, serde::Serialize)]
pub struct ContentNode {
    pub id: String,
    pub kind: String,
    pub position: i64,
    pub number: Option<String>,
    pub title: Option<String>,
    pub release_date: Option<String>,
    pub duration_min: Option<i64>,
    pub page_count: Option<i64>,
    pub synopsis: Option<String>,
    pub is_special: bool,
    pub children: Vec<ContentNode>,
}

impl ContentNode {
    fn from_record(record: NodeRecord, children: Vec<ContentNode>) -> Self {
        Self {
            id: record.id,
            kind: record.kind,
            position: record.position,
            number: record.number,
            title: record.title,
            release_date: record.release_date,
            duration_min: record.duration_min,
            page_count: record.page_count,
            synopsis: record.synopsis,
            is_special: record.is_special,
            children,
        }
    }
}

/// Content-node use-cases.
pub struct NodeService {
    pool: SqlitePool,
}

impl NodeService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Assemble the full content tree for one media. Roots (and every sibling
    /// group) are ordered by position, then id. Media without rows — or unknown
    /// ids — resolve to an empty tree.
    pub async fn tree_for_media(&self, media_id: &str) -> Result<Vec<ContentNode>, AppError> {
        let rows = list_by_media(&self.pool, media_id).await?;
        let mut by_parent: HashMap<Option<String>, Vec<NodeRecord>> = HashMap::new();
        for row in rows {
            by_parent
                .entry(row.parent_id.clone())
                .or_default()
                .push(row);
        }

        fn children_of(
            by_parent: &HashMap<Option<String>, Vec<NodeRecord>>,
            parent: Option<&str>,
        ) -> Vec<ContentNode> {
            let mut records = by_parent
                .get(&parent.map(str::to_string))
                .cloned()
                .unwrap_or_default();
            records.sort_by(|a, b| a.position.cmp(&b.position).then_with(|| a.id.cmp(&b.id)));
            records
                .into_iter()
                .map(|record| {
                    let children = children_of(by_parent, Some(record.id.as_str()));
                    ContentNode::from_record(record, children)
                })
                .collect()
        }

        Ok(children_of(&by_parent, None))
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
    ) -> NodeRecord {
        NodeRecord {
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
                content_type: "anime".into(),
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

    #[tokio::test]
    async fn tree_for_media_builds_nested_ordered_tree() {
        let (pool, path) = migrated_pool("node_service_tree.db").await;
        seed_media(&pool, "m-1").await;
        node::create(&pool, &sample_node("v1", "m-1", None, "volume", 1))
            .await
            .expect("create v1");
        node::create(&pool, &sample_node("c1", "m-1", Some("v1"), "chapter", 1))
            .await
            .expect("create c1");
        node::create(&pool, &sample_node("c2", "m-1", Some("v1"), "chapter", 2))
            .await
            .expect("create c2");
        node::create(&pool, &sample_node("v2", "m-1", None, "volume", 2))
            .await
            .expect("create v2");
        node::create(&pool, &sample_node("c3", "m-1", Some("v2"), "chapter", 1))
            .await
            .expect("create c3");

        let service = NodeService::new(pool.clone());
        let tree = service.tree_for_media("m-1").await.expect("tree");
        assert_eq!(tree.len(), 2);
        assert_eq!(tree[0].id, "v1");
        assert_eq!(tree[1].id, "v2");
        assert_eq!(tree[0].children.len(), 2);
        assert_eq!(tree[0].children[0].id, "c1");
        assert_eq!(tree[0].children[1].id, "c2");
        assert_eq!(tree[1].children.len(), 1);
        assert_eq!(tree[1].children[0].id, "c3");
        assert!(tree[0].children[0].children.is_empty());

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn tree_for_media_flat_nodes_are_roots() {
        let (pool, path) = migrated_pool("node_service_flat.db").await;
        seed_media(&pool, "m-1").await;
        node::create(&pool, &sample_node("e1", "m-1", None, "episode", 1))
            .await
            .expect("create e1");
        node::create(&pool, &sample_node("e2", "m-1", None, "episode", 2))
            .await
            .expect("create e2");

        let service = NodeService::new(pool.clone());
        let tree = service.tree_for_media("m-1").await.expect("tree");
        assert_eq!(tree.len(), 2);
        assert_eq!(tree[0].id, "e1");
        assert_eq!(tree[1].id, "e2");
        assert!(tree[0].children.is_empty());

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn tree_for_media_orders_siblings_by_position_then_id() {
        let (pool, path) = migrated_pool("node_service_order.db").await;
        seed_media(&pool, "m-1").await;
        node::create(&pool, &sample_node("b", "m-1", None, "chapter", 1))
            .await
            .expect("create");
        node::create(&pool, &sample_node("a", "m-1", None, "chapter", 1))
            .await
            .expect("create");
        node::create(&pool, &sample_node("c", "m-1", None, "chapter", 3))
            .await
            .expect("create");

        let service = NodeService::new(pool.clone());
        let tree = service.tree_for_media("m-1").await.expect("tree");
        assert_eq!(
            tree.iter().map(|node| node.id.as_str()).collect::<Vec<_>>(),
            vec!["a", "b", "c"],
            "same position falls back to id order"
        );

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn tree_for_media_maps_display_fields() {
        let (pool, path) = migrated_pool("node_service_fields.db").await;
        seed_media(&pool, "m-1").await;
        let mut episode = sample_node("e1", "m-1", None, "episode", 1);
        episode.number = Some("12.5".into());
        episode.title = Some("The Beginning".into());
        episode.release_date = Some("2026-01-01".into());
        episode.duration_min = Some(24);
        episode.synopsis = Some("A quiet prologue.".into());
        episode.is_special = true;
        node::create(&pool, &episode).await.expect("create episode");

        let service = NodeService::new(pool.clone());
        let tree = service.tree_for_media("m-1").await.expect("tree");
        let node = &tree[0];
        assert_eq!(node.number.as_deref(), Some("12.5"));
        assert_eq!(node.title.as_deref(), Some("The Beginning"));
        assert_eq!(node.release_date.as_deref(), Some("2026-01-01"));
        assert_eq!(node.duration_min, Some(24));
        assert_eq!(node.synopsis.as_deref(), Some("A quiet prologue."));
        assert!(node.is_special);

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn tree_for_media_unknown_media_resolves_empty() {
        let (pool, path) = migrated_pool("node_service_missing.db").await;
        let service = NodeService::new(pool.clone());
        let tree = service.tree_for_media("m-nope").await.expect("tree");
        assert!(tree.is_empty());
        pool.close().await;
        cleanup_files(&path);
    }
}
