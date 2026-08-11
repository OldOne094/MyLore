//! Content-node repository (MISSION-019).
//!
//! Handles the `content_node` tree. Cross-row invariants that SQLite cannot
//! express (a parent must belong to the same media; no cycles) are enforced
//! here by the validators in `infrastructure::content_node` before any write.

use sqlx::sqlite::{SqlitePool, SqliteRow};
use sqlx::Row;

use crate::error::AppError;
use crate::infrastructure::content_node::{ensure_acyclic, validate_parent_belongs_to_media};

/// A content-node row (structure only; per-node progress lives in `tracking`).
#[derive(Debug, Clone)]
pub struct NodeRecord {
    pub id: String,
    pub media_id: String,
    pub parent_id: Option<String>,
    pub kind: String,
    pub position: i64,
    pub number: Option<String>,
    pub title: Option<String>,
    pub release_date: Option<String>,
    pub duration_min: Option<i64>,
    pub page_count: Option<i64>,
    pub synopsis: Option<String>,
    pub external_id: Option<String>,
    pub is_special: bool,
    pub created_at: String,
}

const NODE_COLUMNS: &str = "id, media_id, parent_id, kind, position, number, title, \
     release_date, duration_min, page_count, synopsis, external_id, is_special, created_at";

/// Insert a node after validating parent ownership and acyclicity.
pub async fn create(pool: &SqlitePool, node: &NodeRecord) -> Result<(), AppError> {
    validate_parent_belongs_to_media(pool, &node.media_id, node.parent_id.as_deref()).await?;
    ensure_acyclic(pool, &node.id, node.parent_id.as_deref()).await?;

    sqlx::query(&format!(
        "INSERT INTO content_node ({NODE_COLUMNS}) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    ))
    .bind(&node.id)
    .bind(&node.media_id)
    .bind(&node.parent_id)
    .bind(&node.kind)
    .bind(node.position)
    .bind(&node.number)
    .bind(&node.title)
    .bind(&node.release_date)
    .bind(node.duration_min)
    .bind(node.page_count)
    .bind(&node.synopsis)
    .bind(&node.external_id)
    .bind(node.is_special)
    .bind(&node.created_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Move a node under a new parent (or make it a root). Validates acyclicity;
/// parent ownership is validated too when a parent is given.
pub async fn reparent(
    pool: &SqlitePool,
    node_id: &str,
    new_parent_id: Option<&str>,
) -> Result<(), AppError> {
    if let Some(parent_id) = new_parent_id {
        let (media_id,): (String,) =
            sqlx::query_as("SELECT media_id FROM content_node WHERE id = ?")
                .bind(node_id)
                .fetch_one(pool)
                .await?;
        validate_parent_belongs_to_media(pool, &media_id, Some(parent_id)).await?;
        ensure_acyclic(pool, node_id, Some(parent_id)).await?;
    }

    sqlx::query("UPDATE content_node SET parent_id = ? WHERE id = ?")
        .bind(new_parent_id)
        .bind(node_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Update structural columns of one node.
pub async fn update(pool: &SqlitePool, node: &NodeRecord) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE content_node SET
           kind = ?, position = ?, number = ?, title = ?, release_date = ?,
           duration_min = ?, page_count = ?, synopsis = ?, external_id = ?, is_special = ?
         WHERE id = ?",
    )
    .bind(&node.kind)
    .bind(node.position)
    .bind(&node.number)
    .bind(&node.title)
    .bind(&node.release_date)
    .bind(node.duration_min)
    .bind(node.page_count)
    .bind(&node.synopsis)
    .bind(&node.external_id)
    .bind(node.is_special)
    .bind(&node.id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Delete a node; descendants cascade via the self-referencing FK.
pub async fn delete(pool: &SqlitePool, id: &str) -> Result<(), AppError> {
    sqlx::query("DELETE FROM content_node WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Fetch one node (or `None`).
pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<NodeRecord>, AppError> {
    let row = sqlx::query(&format!(
        "SELECT {NODE_COLUMNS} FROM content_node WHERE id = ?"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(row_to_node))
}

/// Direct children of `parent_id` within `media_id`, ordered by position.
pub async fn children(
    pool: &SqlitePool,
    media_id: &str,
    parent_id: Option<&str>,
) -> Result<Vec<NodeRecord>, AppError> {
    let rows = sqlx::query(&format!(
        "SELECT {NODE_COLUMNS} FROM content_node
         WHERE media_id = ? AND parent_id IS ? ORDER BY position, id"
    ))
    .bind(media_id)
    .bind(parent_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(row_to_node).collect())
}

/// All nodes of a media, ordered for tree walks (position, then id).
pub async fn list_by_media(pool: &SqlitePool, media_id: &str) -> Result<Vec<NodeRecord>, AppError> {
    let rows = sqlx::query(&format!(
        "SELECT {NODE_COLUMNS} FROM content_node
         WHERE media_id = ? ORDER BY position, id"
    ))
    .bind(media_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(row_to_node).collect())
}

fn row_to_node(row: SqliteRow) -> NodeRecord {
    let get = |idx: usize| -> Option<String> { row.get(idx) };
    NodeRecord {
        id: get(0).expect("id"),
        media_id: get(1).expect("media_id"),
        parent_id: get(2),
        kind: get(3).expect("kind"),
        position: row.get(4),
        number: get(5),
        title: get(6),
        release_date: get(7),
        duration_min: row.get(8),
        page_count: row.get(9),
        synopsis: get(10),
        external_id: get(11),
        is_special: row.get::<i64, _>(12) != 0,
        created_at: get(13).expect("created_at"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::repositories::media;
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

    async fn seed_media(pool: &SqlitePool, id: &str) {
        media::create(
            pool,
            &media::MediaRecord {
                id: id.to_string(),
                content_type: "novel".into(),
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
    async fn create_roundtrip_and_children_ordering() {
        let (pool, path) = migrated_pool("node_repo_basic.db").await;
        seed_media(&pool, "m-1").await;
        create(&pool, &sample_node("v1", "m-1", None, "volume", 1))
            .await
            .expect("create volume");
        create(&pool, &sample_node("c1", "m-1", Some("v1"), "chapter", 1))
            .await
            .expect("create ch1");
        create(&pool, &sample_node("c2", "m-1", Some("v1"), "chapter", 2))
            .await
            .expect("create ch2");
        create(&pool, &sample_node("v2", "m-1", None, "volume", 2))
            .await
            .expect("create volume 2");

        let roots = children(&pool, "m-1", None).await.expect("roots");
        assert_eq!(roots.len(), 2);
        assert_eq!(roots[0].id, "v1");

        let chapters = children(&pool, "m-1", Some("v1")).await.expect("chapters");
        assert_eq!(chapters.len(), 2);
        assert_eq!(chapters[0].id, "c1");
        assert_eq!(chapters[1].id, "c2");

        let all = list_by_media(&pool, "m-1").await.expect("all");
        assert_eq!(all.len(), 4);

        let got = get(&pool, "c1").await.expect("get").unwrap();
        assert_eq!(got.parent_id.as_deref(), Some("v1"));
        assert!(get(&pool, "nope").await.expect("get").is_none());
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn parent_must_belong_to_same_media() {
        let (pool, path) = migrated_pool("node_repo_cross.db").await;
        seed_media(&pool, "m-1").await;
        seed_media(&pool, "m-2").await;
        create(&pool, &sample_node("v1", "m-1", None, "volume", 1))
            .await
            .expect("create");

        let result = create(&pool, &sample_node("c1", "m-2", Some("v1"), "chapter", 1)).await;
        assert!(
            matches!(result, Err(AppError::Validation(_))),
            "cross-media parent must be rejected"
        );
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn reparent_rejects_cycles() {
        let (pool, path) = migrated_pool("node_repo_cycle.db").await;
        seed_media(&pool, "m-1").await;
        create(&pool, &sample_node("n1", "m-1", None, "node", 1))
            .await
            .expect("create n1");
        create(&pool, &sample_node("n2", "m-1", Some("n1"), "node", 2))
            .await
            .expect("create n2");
        create(&pool, &sample_node("n3", "m-1", Some("n2"), "node", 3))
            .await
            .expect("create n3");

        let result = reparent(&pool, "n1", Some("n3")).await;
        assert!(
            matches!(result, Err(AppError::Validation(_))),
            "moving a node under its own descendant must be rejected"
        );

        reparent(&pool, "n3", None)
            .await
            .expect("move to root is fine");
        let got = get(&pool, "n3").await.expect("get").unwrap();
        assert_eq!(got.parent_id, None);
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn delete_cascades_subtree() {
        let (pool, path) = migrated_pool("node_repo_delete.db").await;
        seed_media(&pool, "m-1").await;
        create(&pool, &sample_node("v1", "m-1", None, "volume", 1))
            .await
            .expect("create");
        create(&pool, &sample_node("c1", "m-1", Some("v1"), "chapter", 1))
            .await
            .expect("create");
        create(&pool, &sample_node("c2", "m-1", Some("v1"), "chapter", 2))
            .await
            .expect("create");

        delete(&pool, "v1").await.expect("delete volume");

        let all = list_by_media(&pool, "m-1").await.expect("all");
        assert!(all.is_empty(), "descendants cascade with their parent");
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn update_changes_structural_fields() {
        let (pool, path) = migrated_pool("node_repo_update.db").await;
        seed_media(&pool, "m-1").await;
        let mut node = sample_node("n1", "m-1", None, "chapter", 1);
        create(&pool, &node).await.expect("create");

        node.title = Some("The Beginning".into());
        node.position = 2;
        node.is_special = true;
        update(&pool, &node).await.expect("update");

        let got = get(&pool, "n1").await.expect("get").unwrap();
        assert_eq!(got.title.as_deref(), Some("The Beginning"));
        assert_eq!(got.position, 2);
        assert!(got.is_special);
        pool.close().await;
        cleanup_files(&path);
    }
}
