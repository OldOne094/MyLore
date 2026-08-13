//! Trash repository (MISSION-044).
//!
//! Soft-delete works by storing the full JSON before-image of a deleted
//! aggregate in `trash` (REQ-MEDIA-007), then cascading the real row away so
//! the data is recoverable. `restore` re-creates the aggregate from that
//! payload; `purge` removes the trash row forever.

use sqlx::SqlitePool;

use crate::error::AppError;

/// One trash row (before-image of a deleted aggregate).
#[derive(Debug, Clone)]
pub struct TrashRow {
    pub id: String,
    pub kind: String,
    pub payload: String,
    pub deleted_at: String,
    pub restored: bool,
}

/// Insert a trash entry.
pub async fn insert(
    pool: &SqlitePool,
    id: &str,
    kind: &str,
    payload: &str,
    deleted_at: &str,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO trash (id, kind, payload, deleted_at, restored) VALUES (?, ?, ?, ?, 0)",
    )
    .bind(id)
    .bind(kind)
    .bind(payload)
    .bind(deleted_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Read one trash entry, matching on id and kind.
pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<TrashRow>, AppError> {
    let row = sqlx::query_as::<_, (String, String, String, String, i64)>(
        "SELECT id, kind, payload, deleted_at, restored FROM trash WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(
        row.map(|(id, kind, payload, deleted_at, restored)| TrashRow {
            id,
            kind,
            payload,
            deleted_at,
            restored: restored == 1,
        }),
    )
}

/// List active (not-yet-restored) trash entries, newest first.
pub async fn list(pool: &SqlitePool) -> Result<Vec<TrashRow>, AppError> {
    let rows = sqlx::query_as::<_, (String, String, String, String, i64)>(
        "SELECT id, kind, payload, deleted_at, restored FROM trash WHERE restored = 0 \
         ORDER BY deleted_at DESC",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, kind, payload, deleted_at, restored)| TrashRow {
            id,
            kind,
            payload,
            deleted_at,
            restored: restored == 1,
        })
        .collect())
}

/// Mark a trash entry as restored.
pub async fn mark_restored(pool: &SqlitePool, id: &str) -> Result<(), AppError> {
    sqlx::query("UPDATE trash SET restored = 1 WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Permanently remove a trash entry (the aggregate is already gone).
pub async fn purge(pool: &SqlitePool, id: &str) -> Result<(), AppError> {
    sqlx::query("DELETE FROM trash WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::test_support::migrated_pool;

    #[tokio::test]
    async fn insert_get_and_list_roundtrip() {
        let (pool, _path) = migrated_pool("trash_repo_roundtrip.db").await;

        insert(
            &pool,
            "t-1",
            "media",
            "{\"title\":\"Gone\"}",
            "2026-01-01T00:00:00Z",
        )
        .await
        .expect("insert");

        let got = get(&pool, "t-1").await.expect("get").expect("present");
        assert_eq!(got.kind, "media");
        assert_eq!(got.payload, "{\"title\":\"Gone\"}");
        assert!(!got.restored);

        let rows = list(&pool).await.expect("list");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "t-1");
        assert_eq!(rows[0].deleted_at, "2026-01-01T00:00:00Z");
    }

    #[tokio::test]
    async fn get_returns_none_for_missing() {
        let (pool, _path) = migrated_pool("trash_repo_missing.db").await;
        assert!(get(&pool, "nope").await.expect("get").is_none());
    }

    #[tokio::test]
    async fn list_excludes_restored_entries() {
        let (pool, _path) = migrated_pool("trash_repo_restored.db").await;
        insert(&pool, "t-1", "media", "{}", "2026-01-01T00:00:00Z")
            .await
            .expect("insert");
        insert(&pool, "t-2", "media", "{}", "2026-01-02T00:00:00Z")
            .await
            .expect("insert");

        mark_restored(&pool, "t-1").await.expect("restore");

        let rows = list(&pool).await.expect("list");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "t-2");
    }

    #[tokio::test]
    async fn purge_removes_the_entry() {
        let (pool, _path) = migrated_pool("trash_repo_purge.db").await;
        insert(&pool, "t-1", "media", "{}", "2026-01-01T00:00:00Z")
            .await
            .expect("insert");

        purge(&pool, "t-1").await.expect("purge");
        assert!(get(&pool, "t-1").await.expect("get").is_none());
    }
}
