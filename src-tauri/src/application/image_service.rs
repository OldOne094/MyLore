//! Cover/banner image pipeline (MISSION-062).
//!
//! `ImageService` resolves an `asset` row to a local, cached file — the
//! download/cache + broken-URL + cache-policy brain. Resolving an asset never
//! fails on a bad URL: it flips the row's status instead (`cached` / `failed`
//! / `missing`) and reports the outcome as an `AssetView` so the UI can pick
//! a cached file or fall back to a placeholder.
//!
//! Cache policy:
//! - `cached` + file on disk → serve local, no network.
//! - `cached` but file gone → treat as `remote` (re-download).
//! - `remote` → download now.
//! - `failed` → retry only after a cooldown (`FAILED_RETRY_COOLDOWN`) measured
//!   from `last_fetched_at`; within the cooldown the failure is served as-is.
//! - `missing` → permanent broken URL; never auto-retried.

use std::time::Duration;

use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use tracing::{info, warn};

use crate::error::AppError;
use crate::infrastructure::images::{ImageCache, ImageClient, ImageError};
use crate::infrastructure::repositories::asset as asset_repo;
use crate::infrastructure::repositories::asset::AssetRecord;

/// How long a `failed` asset must wait before a resolve retries it.
pub const FAILED_RETRY_COOLDOWN: Duration = Duration::from_secs(60 * 60);

/// Cache-policy timestamps older than now trigger a retry of a `failed` asset.
fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

fn parse_timestamp(raw: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

/// IPC-facing asset snapshot.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AssetView {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub local_path: Option<String>,
    pub remote_url: Option<String>,
    pub mime_type: Option<String>,
}

impl From<AssetRecord> for AssetView {
    fn from(a: AssetRecord) -> Self {
        AssetView {
            id: a.id,
            kind: a.kind,
            status: a.status,
            local_path: a.local_path,
            remote_url: a.remote_url,
            mime_type: a.mime_type,
        }
    }
}

/// The image download/cache pipeline.
pub struct ImageService {
    pool: SqlitePool,
    client: ImageClient,
    cache: ImageCache,
}

impl ImageService {
    pub fn new(pool: SqlitePool, cache_dir: impl Into<std::path::PathBuf>) -> Self {
        Self {
            pool,
            client: ImageClient::new(),
            cache: ImageCache::new(cache_dir),
        }
    }

    /// Resolve one asset to a view, applying the cache policy (downloads when
    /// needed). Missing rows are an error — callers only resolve known ids.
    pub async fn resolve(&self, asset_id: &str) -> Result<AssetView, AppError> {
        let Some(asset) = asset_repo::get(&self.pool, asset_id).await? else {
            return Err(AppError::validation(format!("asset {asset_id} not found")));
        };
        let view = self.ensure_local(&asset).await?;
        info!(asset_id, status = %view.status, "asset resolved");
        Ok(view)
    }

    /// Resolve many assets (deduped). Unlike `resolve`, unknown ids are skipped
    /// so the batch never fails on a stale media reference.
    pub async fn resolve_many(&self, ids: &[String]) -> Result<Vec<AssetView>, AppError> {
        let mut seen = std::collections::HashSet::new();
        let unique: Vec<&String> = ids.iter().filter(|id| seen.insert(id.as_str())).collect();
        let mut views = Vec::with_capacity(unique.len());
        for id in unique {
            if let Ok(view) = self.resolve(id).await {
                views.push(view);
            }
        }
        Ok(views)
    }

    /// Apply the cache policy; returns a view describing what the UI can use.
    async fn ensure_local(&self, asset: &AssetRecord) -> Result<AssetView, AppError> {
        match asset.status.as_str() {
            "cached" => {
                if let Some(path) = &asset.local_path {
                    if std::path::Path::new(path).exists() {
                        return Ok(asset.clone().into());
                    }
                }
                // File vanished → re-download.
                self.download(asset).await
            }
            "missing" => Ok(asset.clone().into()),
            "failed" => {
                let cooled_off = asset
                    .last_fetched_at
                    .as_deref()
                    .and_then(parse_timestamp)
                    .map(|last| {
                        last + chrono::Duration::from_std(FAILED_RETRY_COOLDOWN).expect("cooldown")
                            < Utc::now()
                    })
                    .unwrap_or(true);
                if cooled_off {
                    self.download(asset).await
                } else {
                    Ok(asset.clone().into())
                }
            }
            // "remote" (or any unknown state) → download now.
            _ => self.download(asset).await,
        }
    }

    /// Attempt a download and record the outcome on the asset row.
    async fn download(&self, asset: &AssetRecord) -> Result<AssetView, AppError> {
        let Some(url) = asset.remote_url.as_deref() else {
            // No remote URL → permanently broken; nothing to retry.
            self.mark(&asset.id, "missing", None, None, None).await?;
            return self.reload(&asset.id).await;
        };

        match self.client.fetch(url).await {
            Ok(image) => {
                let path = self.cache.save(&asset.id, &image.mime_type, &image.bytes);
                match path {
                    Ok(path) => {
                        let path_str = path.to_string_lossy().into_owned();
                        self.mark(
                            &asset.id,
                            "cached",
                            Some(&path_str),
                            Some(&image.mime_type),
                            image.etag.as_deref(),
                        )
                        .await?;
                    }
                    Err(error) => {
                        warn!(asset_id = %asset.id, %error, "image cache write failed");
                        self.mark(&asset.id, "failed", None, None, None).await?;
                    }
                }
            }
            Err(ImageError::NotFound) => {
                info!(asset_id = %asset.id, "cover URL is broken; marked missing");
                self.mark(&asset.id, "missing", None, None, None).await?;
            }
            Err(ImageError::Transient(error)) => {
                warn!(asset_id = %asset.id, %error, "cover download failed; will retry after cooldown");
                self.mark(&asset.id, "failed", None, None, None).await?;
            }
        }

        self.reload(&asset.id).await
    }

    /// Persist a fetch outcome (status + optional cached metadata).
    async fn mark(
        &self,
        id: &str,
        status: &str,
        local_path: Option<&str>,
        mime_type: Option<&str>,
        etag: Option<&str>,
    ) -> Result<(), AppError> {
        asset_repo::update_fetch(
            &self.pool,
            id,
            status,
            local_path,
            mime_type,
            etag,
            Some(&now_rfc3339()),
        )
        .await
    }

    /// Re-read an asset after a state change.
    async fn reload(&self, id: &str) -> Result<AssetView, AppError> {
        let asset = asset_repo::get(&self.pool, id)
            .await?
            .ok_or_else(|| AppError::validation(format!("asset {id} not found")))?;
        Ok(asset.into())
    }
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::infrastructure::repositories::asset as asset_repo;
    use crate::infrastructure::test_support::{cleanup_files, migrated_pool};

    const CACHE_SUBDIR: &str = "mylore-img-service";

    struct Harness {
        pool: sqlx::SqlitePool,
        db_path: std::path::PathBuf,
        cache_dir: std::path::PathBuf,
        service: ImageService,
    }

    impl Harness {
        async fn new(name: &str) -> Self {
            let (pool, db_path) = migrated_pool(name).await;
            let cache_dir = std::env::temp_dir().join(CACHE_SUBDIR).join(name);
            let _ = std::fs::remove_dir_all(&cache_dir);
            let service = ImageService::new(pool.clone(), &cache_dir);
            Harness {
                pool,
                db_path,
                cache_dir,
                service,
            }
        }

        async fn insert(&self, id: &str, status: &str, remote_url: Option<&str>) {
            asset_repo::insert(
                &self.pool,
                &AssetRecord {
                    id: id.to_string(),
                    kind: "cover".to_string(),
                    remote_url: remote_url.map(str::to_string),
                    local_path: None,
                    status: status.to_string(),
                    mime_type: None,
                    width: None,
                    height: None,
                    etag: None,
                    last_fetched_at: None,
                    created_at: "2026-01-01".to_string(),
                },
            )
            .await
            .expect("insert asset");
        }

        async fn resolve_ok(&self, id: &str) -> AssetView {
            self.service.resolve(id).await.expect("resolve")
        }
    }

    impl Drop for Harness {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.cache_dir);
        }
    }

    #[tokio::test]
    async fn remote_asset_downloads_and_caches() {
        let h = Harness::new("download.db").await;
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/cover.jpg"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "image/jpeg")
                    .insert_header("etag", "\"etag-1\"")
                    .set_body_bytes(b"jpeg-bytes"),
            )
            .mount(&server)
            .await;
        h.insert(
            "a-1",
            "remote",
            Some(&format!("{}/cover.jpg", server.uri())),
        )
        .await;

        let view = h.resolve_ok("a-1").await;
        assert_eq!(view.status, "cached");
        assert_eq!(view.mime_type.as_deref(), Some("image/jpeg"));
        assert!(
            view.local_path
                .as_deref()
                .is_some_and(|p| std::path::Path::new(p).exists()),
            "cached file must exist on disk"
        );

        let stored = asset_repo::get(&h.pool, "a-1").await.expect("get").unwrap();
        assert_eq!(stored.status, "cached");
        assert_eq!(stored.etag.as_deref(), Some("\"etag-1\""));
        assert!(stored.last_fetched_at.is_some());
        h.pool.close().await;
        cleanup_files(&h.db_path);
    }

    #[tokio::test]
    async fn cached_asset_serves_without_network() {
        let h = Harness::new("served.db").await;
        let server = MockServer::start().await;
        let mount = Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(500))
            .expect(0) // must never hit the wire again
            .mount_as_scoped(&server)
            .await;

        // Seed a cached asset whose file already exists.
        let cache_path = h
            .service
            .cache
            .save("a-1", "image/jpeg", b"jpeg-bytes")
            .expect("seed cache file");
        asset_repo::insert(
            &h.pool,
            &AssetRecord {
                id: "a-1".to_string(),
                kind: "cover".to_string(),
                remote_url: Some(format!("{}/gone", server.uri())),
                local_path: Some(cache_path.to_string_lossy().into_owned()),
                status: "cached".to_string(),
                mime_type: Some("image/jpeg".into()),
                width: None,
                height: None,
                etag: Some("\"etag-1\"".into()),
                last_fetched_at: Some("2026-01-01".into()),
                created_at: "2026-01-01".to_string(),
            },
        )
        .await
        .expect("seed asset");

        let view = h.resolve_ok("a-1").await;
        assert_eq!(view.status, "cached");
        assert!(view.local_path.is_some());
        drop(mount); // verifies `.expect(0)`
        h.pool.close().await;
        cleanup_files(&h.db_path);
    }

    #[tokio::test]
    async fn missing_file_on_cached_asset_re_downloads() {
        let h = Harness::new("re-download.db").await;
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "image/png")
                    .set_body_bytes(b"png-bytes"),
            )
            .mount(&server)
            .await;
        asset_repo::insert(
            &h.pool,
            &AssetRecord {
                id: "a-1".to_string(),
                kind: "cover".to_string(),
                remote_url: Some(format!("{}/cover.png", server.uri())),
                local_path: Some("C:/definitely/missing/file.jpg".to_string()),
                status: "cached".to_string(),
                mime_type: Some("image/jpeg".into()),
                width: None,
                height: None,
                etag: None,
                last_fetched_at: None,
                created_at: "2026-01-01".to_string(),
            },
        )
        .await
        .expect("seed asset");

        let view = h.resolve_ok("a-1").await;
        assert_eq!(view.status, "cached");
        assert_eq!(view.mime_type.as_deref(), Some("image/png"), "re-fetched");
        h.pool.close().await;
        cleanup_files(&h.db_path);
    }

    #[tokio::test]
    async fn broken_url_marks_missing_and_is_not_retried() {
        let h = Harness::new("broken.db").await;
        let server = MockServer::start().await;
        let mount = Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .expect(1)
            .mount_as_scoped(&server)
            .await;
        h.insert("a-1", "remote", Some(&format!("{}/gone", server.uri())))
            .await;

        let first = h.resolve_ok("a-1").await;
        assert_eq!(first.status, "missing");

        // A second resolve must not hit the wire: missing is permanent.
        let second = h.resolve_ok("a-1").await;
        assert_eq!(second.status, "missing");
        drop(mount); // verifies `.expect(1)`
        h.pool.close().await;
        cleanup_files(&h.db_path);
    }

    #[tokio::test]
    async fn transient_failure_is_retried_after_cooldown() {
        let h = Harness::new("cooldown.db").await;
        let server = MockServer::start().await;
        let mount = Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(503))
            .expect(1)
            .mount_as_scoped(&server)
            .await;
        h.insert("a-1", "remote", Some(&format!("{}/down", server.uri())))
            .await;

        let first = h.resolve_ok("a-1").await;
        assert_eq!(first.status, "failed");

        // Within the cooldown, resolves serve the failure without another fetch.
        let within = h.resolve_ok("a-1").await;
        assert_eq!(within.status, "failed");
        drop(mount); // verifies `.expect(1)`: cooldown held back the second fetch

        // Once the cooldown passes, a resolve retries.
        let old = (Utc::now()
            - chrono::Duration::from_std(FAILED_RETRY_COOLDOWN).unwrap()
            - chrono::Duration::hours(1))
        .to_rfc3339();
        asset_repo::update_fetch(&h.pool, "a-1", "failed", None, None, None, Some(&old))
            .await
            .expect("age the failure");
        let mount2 = Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "image/webp")
                    .set_body_bytes(b"webp-bytes"),
            )
            .expect(1)
            .mount_as_scoped(&server)
            .await;
        let retried = h.resolve_ok("a-1").await;
        assert_eq!(retried.status, "cached");
        assert_eq!(retried.mime_type.as_deref(), Some("image/webp"));
        drop(mount2); // verifies `.expect(1)`
        h.pool.close().await;
        cleanup_files(&h.db_path);
    }

    #[tokio::test]
    async fn asset_without_remote_url_is_missing() {
        let h = Harness::new("no-url.db").await;
        h.insert("a-1", "remote", None).await;
        let view = h.resolve_ok("a-1").await;
        assert_eq!(view.status, "missing");
        h.pool.close().await;
        cleanup_files(&h.db_path);
    }

    #[tokio::test]
    async fn resolve_many_skips_unknown_and_dedupes() {
        let h = Harness::new("batch.db").await;
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/cover.jpg"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "image/jpeg")
                    .set_body_bytes(b"jpeg-bytes"),
            )
            .mount(&server)
            .await;
        h.insert(
            "a-1",
            "remote",
            Some(&format!("{}/cover.jpg", server.uri())),
        )
        .await;

        let views = h
            .service
            .resolve_many(&["a-1".into(), "missing-id".into(), "a-1".into()])
            .await
            .expect("batch");
        assert_eq!(views.len(), 1, "deduped and skipped unknown");
        assert_eq!(views[0].status, "cached");
        h.pool.close().await;
        cleanup_files(&h.db_path);
    }

    #[tokio::test]
    async fn resolve_unknown_asset_is_an_error() {
        let h = Harness::new("unknown.db").await;
        let result = h.service.resolve("nope").await;
        assert!(result.is_err());
        h.pool.close().await;
        cleanup_files(&h.db_path);
    }
}
