//! Asset repository (MISSION-019).

use sqlx::sqlite::{SqlitePool, SqliteRow};
use sqlx::Row;

use crate::error::AppError;

/// A media asset (cover/banner/avatar/node image).
#[derive(Debug, Clone)]
pub struct AssetRecord {
    pub id: String,
    pub kind: String,
    pub remote_url: Option<String>,
    pub local_path: Option<String>,
    pub status: String,
    pub mime_type: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub etag: Option<String>,
    pub last_fetched_at: Option<String>,
    pub created_at: String,
}

/// Insert an asset.
pub async fn insert(pool: &SqlitePool, a: &AssetRecord) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO asset
           (id, kind, remote_url, local_path, status, mime_type, width, height, etag,
            last_fetched_at, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&a.id)
    .bind(&a.kind)
    .bind(&a.remote_url)
    .bind(&a.local_path)
    .bind(&a.status)
    .bind(&a.mime_type)
    .bind(a.width)
    .bind(a.height)
    .bind(&a.etag)
    .bind(&a.last_fetched_at)
    .bind(&a.created_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Fetch one asset (or `None`).
pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<AssetRecord>, AppError> {
    let row = sqlx::query(
        "SELECT id, kind, remote_url, local_path, status, mime_type, width, height, etag, \
         last_fetched_at, created_at FROM asset WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(row_to_asset))
}

/// Update the download/fetch state of an asset (cached/failed/missing, path).
pub async fn update_status(
    pool: &SqlitePool,
    id: &str,
    status: &str,
    local_path: Option<&str>,
    last_fetched_at: Option<&str>,
) -> Result<(), AppError> {
    sqlx::query("UPDATE asset SET status = ?, local_path = ?, last_fetched_at = ? WHERE id = ?")
        .bind(status)
        .bind(local_path)
        .bind(last_fetched_at)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Record a completed/failed fetch result (MISSION-062): status, local path,
/// mime type and etag captured from the download.
pub async fn update_fetch(
    pool: &SqlitePool,
    id: &str,
    status: &str,
    local_path: Option<&str>,
    mime_type: Option<&str>,
    etag: Option<&str>,
    last_fetched_at: Option<&str>,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE asset SET status = ?, local_path = ?, mime_type = ?, etag = ?, \
         last_fetched_at = ? WHERE id = ?",
    )
    .bind(status)
    .bind(local_path)
    .bind(mime_type)
    .bind(etag)
    .bind(last_fetched_at)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// List assets by ids (in the given order, deduped) — the batch resolve path.
pub async fn list_by_ids(pool: &SqlitePool, ids: &[String]) -> Result<Vec<AssetRecord>, AppError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut qb = sqlx::QueryBuilder::new(
        "SELECT id, kind, remote_url, local_path, status, mime_type, width, height, etag, \
         last_fetched_at, created_at FROM asset WHERE id IN (",
    );
    let mut separated = qb.separated(", ");
    for id in ids {
        separated.push_bind(id);
    }
    separated.push_unseparated(")");
    Ok(qb.build().fetch_all(pool).await?.into_iter().map(row_to_asset).collect())
}

/// Delete an asset; media cover/banner columns SET NULL via FK.
pub async fn delete(pool: &SqlitePool, id: &str) -> Result<(), AppError> {
    sqlx::query("DELETE FROM asset WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

fn row_to_asset(row: SqliteRow) -> AssetRecord {
    let get = |idx: usize| -> Option<String> { row.get(idx) };
    AssetRecord {
        id: get(0).expect("id"),
        kind: get(1).expect("kind"),
        remote_url: get(2),
        local_path: get(3),
        status: get(4).expect("status"),
        mime_type: get(5),
        width: row.get(6),
        height: row.get(7),
        etag: get(8),
        last_fetched_at: get(9),
        created_at: get(10).expect("created_at"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::test_support::{cleanup_files, migrated_pool};

    fn asset(id: &str) -> AssetRecord {
        AssetRecord {
            id: id.to_string(),
            kind: "cover".to_string(),
            remote_url: Some(format!("https://cdn.example/{id}.jpg")),
            local_path: None,
            status: "remote".to_string(),
            mime_type: Some("image/jpeg".into()),
            width: Some(600),
            height: Some(900),
            etag: None,
            last_fetched_at: None,
            created_at: "2026-01-01".to_string(),
        }
    }

    #[tokio::test]
    async fn insert_status_update_and_delete() {
        let (pool, path) = migrated_pool("asset_repo.db").await;

        insert(&pool, &asset("a-1")).await.expect("insert");
        let got = get(&pool, "a-1").await.expect("get").unwrap();
        assert_eq!(got.kind, "cover");
        assert_eq!(got.width, Some(600));

        update_status(
            &pool,
            "a-1",
            "cached",
            Some("cache/a-1.jpg"),
            Some("2026-01-05"),
        )
        .await
        .expect("update status");
        let got = get(&pool, "a-1").await.expect("get").unwrap();
        assert_eq!(got.status, "cached");
        assert_eq!(got.local_path.as_deref(), Some("cache/a-1.jpg"));

        assert!(get(&pool, "nope").await.expect("get").is_none());
        delete(&pool, "a-1").await.expect("delete");
        assert!(get(&pool, "a-1").await.expect("get").is_none());
        pool.close().await;
        cleanup_files(&path);
    }

    #[tokio::test]
    async fn update_fetch_and_list_by_ids() {
        let (pool, path) = migrated_pool("asset_repo_fetch.db").await;
        insert(&pool, &asset("a-1")).await.expect("insert 1");
        insert(&pool, &asset("a-2")).await.expect("insert 2");

        update_fetch(
            &pool,
            "a-1",
            "cached",
            Some("cache/a-1.jpg"),
            Some("image/jpeg"),
            Some("\"etag-1\""),
            Some("2026-01-05"),
        )
        .await
        .expect("update fetch");
        let got = get(&pool, "a-1").await.expect("get").unwrap();
        assert_eq!(got.status, "cached");
        assert_eq!(got.mime_type.as_deref(), Some("image/jpeg"));
        assert_eq!(got.etag.as_deref(), Some("\"etag-1\""));

        let all = list_by_ids(&pool, &["a-2".to_string(), "a-1".to_string(), "a-1".to_string()])
            .await
            .expect("list by ids");
        assert_eq!(all.len(), 2, "deduped");

        assert!(list_by_ids(&pool, &[]).await.expect("empty").is_empty());

        pool.close().await;
        cleanup_files(&path);
    }
}
