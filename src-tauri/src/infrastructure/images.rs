//! Image download + disk cache infrastructure (MISSION-062).
//!
//! `ImageClient` is a thin reqwest GET that classifies the outcome so the
//! application service can apply its cache policy: a body with its mime type
//! and etag (`FetchedImage`), a broken URL (`NotFound` — 404/410, permanent),
//! or a transient failure (`Transient` — other HTTP statuses and transport
//! errors, retryable). `ImageCache` maps an asset id to a local file under the
//! app data dir (`images/`) and saves bytes atomically (temp + rename).

use std::path::{Path, PathBuf};

use crate::domain::provider::error::ProviderError;

/// A polite, identifiable User-Agent, consistent with the provider clients.
pub const APP_USER_AGENT: &str = concat!(
    "MyLore/",
    env!("CARGO_PKG_VERSION"),
    " (local-first media tracker)"
);

/// A successfully downloaded image body.
#[derive(Debug, Clone)]
pub struct FetchedImage {
    pub bytes: Vec<u8>,
    pub mime_type: String,
    pub etag: Option<String>,
}

/// Why a download did not produce an image. `NotFound` marks a permanently
/// broken URL; `Transient` maps to a retryable failure.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ImageError {
    #[error("remote image not found (broken URL)")]
    NotFound,
    #[error("image download failed: {0}")]
    Transient(String),
}

/// A reqwest-backed image downloader. `Clone` is cheap (one connection pool).
#[derive(Clone)]
pub struct ImageClient {
    http: reqwest::Client,
}

impl Default for ImageClient {
    fn default() -> Self {
        Self::new()
    }
}

impl ImageClient {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .user_agent(APP_USER_AGENT)
            .build()
            .expect("reqwest client builds");
        Self { http }
    }

    /// Test hook: build from an injected client.
    pub fn with_client(http: reqwest::Client) -> Self {
        Self { http }
    }

    /// GET `url`. 200 → the body + content type + etag; 404/410 → `NotFound`
    /// (broken URL, permanent); other HTTP statuses and transport failures →
    /// `Transient` (retryable).
    pub async fn fetch(&self, url: &str) -> Result<FetchedImage, ImageError> {
        let response = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|e| ImageError::Transient(format!("request failed: {e}")))?;

        let status = response.status();
        if status == reqwest::StatusCode::NOT_FOUND || status == reqwest::StatusCode::GONE {
            return Err(ImageError::NotFound);
        }
        if !status.is_success() {
            return Err(ImageError::Transient(format!(
                "server returned HTTP {status}"
            )));
        }

        let etag = response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let mime_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.split(';').next().unwrap_or(value).trim().to_string())
            .unwrap_or_else(|| "application/octet-stream".to_string());

        let bytes = response
            .bytes()
            .await
            .map_err(|e| ImageError::Transient(format!("body read failed: {e}")))?
            .to_vec();

        if bytes.is_empty() {
            return Err(ImageError::Transient("empty image body".to_string()));
        }

        Ok(FetchedImage {
            bytes,
            mime_type,
            etag,
        })
    }
}

/// On-disk cache of downloaded assets under `{data_dir}/images`.
#[derive(Debug, Clone)]
pub struct ImageCache {
    dir: PathBuf,
}

impl ImageCache {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The local file for an asset id with the given mime type.
    pub fn path_for(&self, asset_id: &str, mime_type: &str) -> PathBuf {
        self.dir
            .join(format!("{asset_id}.{}", ext_for_mime(mime_type)))
    }

    /// Save bytes to the cache atomically (temp file + rename), resolving with
    /// the final path. Creates the cache dir when missing.
    pub fn save(&self, asset_id: &str, mime_type: &str, bytes: &[u8]) -> std::io::Result<PathBuf> {
        std::fs::create_dir_all(&self.dir)?;
        let target = self.path_for(asset_id, mime_type);
        let temp = self.dir.join(format!("{asset_id}.tmp"));
        std::fs::write(&temp, bytes)?;
        let _ = std::fs::remove_file(&target);
        std::fs::rename(&temp, &target)?;
        Ok(target)
    }
}

/// File extension for an image mime type (best-effort; non-image mimes fall
/// back to `img` so the asset protocol can still serve them).
pub fn ext_for_mime(mime_type: &str) -> &'static str {
    match mime_type.to_ascii_lowercase().as_str() {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "image/svg+xml" => "svg",
        "image/avif" => "avif",
        "image/bmp" => "bmp",
        _ => "img",
    }
}

impl From<ImageError> for ProviderError {
    fn from(error: ImageError) -> Self {
        match error {
            ImageError::NotFound => ProviderError::NotFound {
                provider: "images".to_string(),
            },
            ImageError::Transient(message) => ProviderError::Transport {
                provider: "images".to_string(),
                message,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    fn client_with(_server: &MockServer) -> ImageClient {
        ImageClient::with_client(
            reqwest::Client::builder()
                .user_agent(APP_USER_AGENT)
                .build()
                .expect("client builds"),
        )
    }

    #[tokio::test]
    async fn fetch_returns_body_content_type_and_etag() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/cover.jpg"))
            .and(header("user-agent", APP_USER_AGENT))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "image/jpeg; charset=utf-8")
                    .insert_header("etag", "\"abc123\"")
                    .set_body_bytes(b"fake-jpeg-bytes"),
            )
            .mount(&server)
            .await;

        let image = client_with(&server)
            .fetch(&format!("{}/cover.jpg", server.uri()))
            .await
            .expect("fetch");
        assert_eq!(image.bytes, b"fake-jpeg-bytes");
        assert_eq!(image.mime_type, "image/jpeg", "params after ; are stripped");
        assert_eq!(image.etag.as_deref(), Some("\"abc123\""));
    }

    #[tokio::test]
    async fn fetch_maps_404_and_410_to_not_found() {
        for status in [404u16, 410] {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .respond_with(ResponseTemplate::new(status))
                .mount(&server)
                .await;
            let result = client_with(&server)
                .fetch(&format!("{}/gone", server.uri()))
                .await;
            assert!(
                matches!(result, Err(ImageError::NotFound)),
                "HTTP {status} must be a broken URL"
            );
        }
    }

    #[tokio::test]
    async fn fetch_maps_5xx_and_empty_body_to_transient() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;
        let result = client_with(&server)
            .fetch(&format!("{}/down", server.uri()))
            .await;
        assert!(
            matches!(result, Err(ImageError::Transient(_))),
            "5xx must be transient"
        );

        let empty = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b""))
            .mount(&empty)
            .await;
        let result = client_with(&empty)
            .fetch(&format!("{}/empty", empty.uri()))
            .await;
        assert!(
            matches!(result, Err(ImageError::Transient(_))),
            "empty body must be transient"
        );
    }

    #[tokio::test]
    async fn fetch_maps_transport_failure_to_transient() {
        let client = ImageClient::new();
        let result = client.fetch("http://127.0.0.1:1/never-reachable").await;
        assert!(
            matches!(result, Err(ImageError::Transient(_))),
            "connection failure must be transient"
        );
    }

    #[test]
    fn ext_for_mime_maps_known_types() {
        assert_eq!(ext_for_mime("image/jpeg"), "jpg");
        assert_eq!(ext_for_mime("image/JPEG"), "jpg", "case-insensitive");
        assert_eq!(ext_for_mime("image/png"), "png");
        assert_eq!(ext_for_mime("image/webp"), "webp");
        assert_eq!(ext_for_mime("image/gif"), "gif");
        assert_eq!(ext_for_mime("image/svg+xml"), "svg");
        assert_eq!(ext_for_mime("image/avif"), "avif");
        assert_eq!(ext_for_mime("image/bmp"), "bmp");
        assert_eq!(ext_for_mime("application/octet-stream"), "img");
    }

    #[test]
    fn cache_saves_and_paths_files() {
        let dir = std::env::temp_dir().join(format!("mylore-img-test-{}", std::process::id()));
        let cache = ImageCache::new(&dir);
        let _ = std::fs::remove_dir_all(&dir);

        let path = cache.save("a-1", "image/jpeg", b"jpeg").expect("save");
        assert_eq!(path.file_name().unwrap(), "a-1.jpg");
        assert_eq!(std::fs::read(&path).unwrap(), b"jpeg");

        assert_eq!(
            cache.path_for("a-2", "image/png").file_name().unwrap(),
            "a-2.png"
        );
        assert!(
            cache.path_for("a-1", "image/jpeg").exists(),
            "saved file is on disk"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
