//! Content-node FK/cross-row validators (MISSION-014).
//!
//! SQLite enforces column-level foreign keys but cannot express the tree
//! invariant that a node's parent must belong to the same media (DOMAIN_MODEL
//! §6), nor that the tree stays acyclic. These helpers run before insert /
//! reparent operations and reject invalid links with `AppError::Validation`.
//!
//! Repositories (MISSION-019) call both helpers before writing a node.

use sqlx::sqlite::SqlitePool;

use crate::error::AppError;

/// Verify `parent_id` (when present) exists and belongs to `media_id`.
///
/// A root node has `parent_id = None` and always passes.
pub async fn validate_parent_belongs_to_media(
    pool: &SqlitePool,
    media_id: &str,
    parent_id: Option<&str>,
) -> Result<(), AppError> {
    let Some(parent_id) = parent_id else {
        return Ok(());
    };

    let (parent_media,): (String,) =
        sqlx::query_as("SELECT media_id FROM content_node WHERE id = ?")
            .bind(parent_id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| AppError::validation(format!("parent node not found: {parent_id}")))?;

    if parent_media != media_id {
        return Err(AppError::validation(format!(
            "parent node {parent_id} belongs to media {parent_media}, expected {media_id}"
        )));
    }
    Ok(())
}

/// Reject parent links that would create a cycle (a node parenting itself, or
/// one of its descendants becoming its ancestor).
///
/// `node_id` is the node being inserted or reparented; for an insert the node
/// does not exist yet, so only the self-parent guard applies.
pub async fn ensure_acyclic(
    pool: &SqlitePool,
    node_id: &str,
    candidate_parent_id: Option<&str>,
) -> Result<(), AppError> {
    let Some(parent_id) = candidate_parent_id else {
        return Ok(());
    };

    if node_id == parent_id {
        return Err(AppError::validation("a node cannot be its own parent"));
    }

    // Walk up from the candidate parent; reaching `node_id` means it is an
    // ancestor of the new parent → cycle.
    let mut cursor = parent_id.to_string();
    loop {
        let (next,): (Option<String>,) =
            sqlx::query_as("SELECT parent_id FROM content_node WHERE id = ?")
                .bind(&cursor)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::validation(format!("parent node not found: {cursor}")))?;

        match next {
            None => return Ok(()),
            Some(next) if next == node_id => {
                return Err(AppError::validation("parent link would create a cycle"));
            }
            Some(next) => cursor = next,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::test_support::{cleanup_files, migrated_pool};

    async fn insert_media(pool: &SqlitePool, id: &str) {
        sqlx::query(
            "INSERT INTO media (id, content_type, title_main, created_at, updated_at)
             VALUES (?, 'novel', 'Test Title', '2026-01-01', '2026-01-01')",
        )
        .bind(id)
        .execute(pool)
        .await
        .expect("insert media");
    }

    async fn insert_node(
        pool: &SqlitePool,
        id: &str,
        media_id: &str,
        parent_id: Option<&str>,
        kind: &str,
        position: i64,
    ) {
        sqlx::query(
            "INSERT INTO content_node (id, media_id, parent_id, kind, position, created_at)
             VALUES (?, ?, ?, ?, ?, '2026-01-01')",
        )
        .bind(id)
        .bind(media_id)
        .bind(parent_id)
        .bind(kind)
        .bind(position)
        .execute(pool)
        .await
        .expect("insert content_node");
    }

    #[tokio::test]
    async fn content_node_schema_and_indexes_created() {
        let (pool, path) = migrated_pool("node_schema.db").await;

        let (found,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'content_node'",
        )
        .fetch_one(&pool)
        .await
        .expect("check content_node table");
        assert_eq!(found, 1, "content_node table should exist after migration");

        for index in ["idx_node_media", "idx_node_parent"] {
            let (found,): (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?",
            )
            .bind(index)
            .fetch_one(&pool)
            .await
            .expect("check index");
            assert_eq!(found, 1, "{index} should exist after migration");
        }

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn parent_must_belong_to_same_media() {
        let (pool, path) = migrated_pool("node_media.db").await;
        insert_media(&pool, "m-1").await;
        insert_media(&pool, "m-2").await;
        insert_node(&pool, "n-root", "m-1", None, "volume", 1).await;

        validate_parent_belongs_to_media(&pool, "m-1", Some("n-root"))
            .await
            .expect("same-media parent is valid");

        let result = validate_parent_belongs_to_media(&pool, "m-2", Some("n-root")).await;
        assert!(
            matches!(result, Err(AppError::Validation(_))),
            "cross-media parent must be rejected"
        );

        let result = validate_parent_belongs_to_media(&pool, "m-1", Some("n-missing")).await;
        assert!(
            matches!(result, Err(AppError::Validation(_))),
            "missing parent must be rejected"
        );

        validate_parent_belongs_to_media(&pool, "m-1", None)
            .await
            .expect("root node (no parent) is valid");

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn parent_link_must_be_acyclic() {
        let (pool, path) = migrated_pool("node_acyclic.db").await;
        insert_media(&pool, "m-1").await;
        insert_node(&pool, "n1", "m-1", None, "chapter", 1).await;
        insert_node(&pool, "n2", "m-1", Some("n1"), "chapter", 2).await;
        insert_node(&pool, "n3", "m-1", Some("n2"), "chapter", 3).await;

        ensure_acyclic(&pool, "n1", Some("n1"))
            .await
            .expect_err("self-parent must be rejected");

        ensure_acyclic(&pool, "n1", Some("n3"))
            .await
            .expect_err("reparenting n1 under its own descendant n3 must be rejected");

        ensure_acyclic(&pool, "n2", Some("n3"))
            .await
            .expect_err("reparenting n2 under n3 must be rejected");

        ensure_acyclic(&pool, "n3", None)
            .await
            .expect("root move is fine");
        ensure_acyclic(&pool, "n3", Some("n1"))
            .await
            .expect("moving n3 under n1 (an ancestor chain) is fine");

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn deleting_media_cascades_to_nodes() {
        let (pool, path) = migrated_pool("node_cascade.db").await;
        insert_media(&pool, "m-1").await;
        insert_node(&pool, "n1", "m-1", None, "volume", 1).await;
        insert_node(&pool, "n2", "m-1", Some("n1"), "chapter", 1).await;

        sqlx::query("DELETE FROM media WHERE id = 'm-1'")
            .execute(&pool)
            .await
            .unwrap();

        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM content_node")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0, "content_node should cascade-delete with media");

        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn kind_enum_is_enforced() {
        let (pool, path) = migrated_pool("node_kind.db").await;
        insert_media(&pool, "m-1").await;

        let result = sqlx::query(
            "INSERT INTO content_node (id, media_id, kind, position, created_at)
             VALUES ('n-bad', 'm-1', 'episode_X', 1, '2026-01-01')",
        )
        .execute(&pool)
        .await;
        assert!(result.is_err(), "invalid kind must be rejected by CHECK");

        pool.close().await;
        cleanup_files(&path);
    }
}
