//! WTR-LAB adapter (web novels, CN/KR → EN) — MISSION-131.
//!
//! Source: wtr-lab.com, a Next.js site hosting translated web novels.
//! Live-verified (2026-08-24): a plain reqwest client passes Cloudflare here
//! (unlike NovelUpdates), so a normal adapter suffices — no bridge needed.
//!
//! Endpoints used (API_PROVIDERS §15):
//! - search:  `GET /api/search?q=` → JSON `{success, data:[…]}`
//! - details: `GET /en/novel/{id}/{slug}` → HTML; metadata parsed from the
//!   embedded `__NEXT_DATA__` JSON script
//! - chapters: `GET /api/chapters/{id}` → JSON `{chapters:[…]}`
//!
//! The provider id is composite `{id}/{slug}`: chapters need only the numeric
//! id while the details page requires the slug.

pub mod client;
mod normalize;
mod response;

use async_trait::async_trait;

use crate::application::providers::config::ProviderConfig;
use crate::domain::enums::ContentType;
use crate::domain::provider::capabilities::{AuthKind, ProviderCapabilities};
use crate::domain::provider::error::ProviderError;
use crate::domain::provider::types::{ProviderCandidate, ProviderMedia, ProviderNode};
use crate::domain::provider::Provider;
use crate::domain::value_objects::ExternalId;

pub const PROVIDER_ID: &str = "wtrlab";
pub const ENDPOINT: &str = "https://wtr-lab.com";

/// Conservative pacing for a scraping source (~2 req/s).
pub const REQUESTS_PER_SEC: f64 = 2.0;

/// The config the coordinator registers WTR-LAB with. The site catalogs
/// translated web novels (CN/KR → EN).
pub fn wtrlab_config() -> ProviderConfig {
    ProviderConfig::new(PROVIDER_ID)
        .with_requests_per_sec(REQUESTS_PER_SEC)
        .with_content_types(vec![ContentType::WebNovel])
}

pub struct WtrLabProvider {
    client: client::WtrLabClient,
    caps: ProviderCapabilities,
}

impl WtrLabProvider {
    pub fn new(client: client::WtrLabClient) -> Self {
        Self {
            client,
            caps: ProviderCapabilities {
                search: true,
                details: true,
                nodes: true,
                related: false,
                reviews: false,
                images: false,
                seasonal: false,
                auth: AuthKind::None,
            },
        }
    }

    /// The numeric serie id is everything the chapters API needs.
    fn numeric_id(provider_id: &str) -> &str {
        provider_id.split('/').next().unwrap_or(provider_id)
    }
}

#[async_trait]
impl Provider for WtrLabProvider {
    fn id(&self) -> String {
        PROVIDER_ID.to_string()
    }

    fn name(&self) -> &str {
        "WTR-LAB"
    }

    fn capabilities(&self) -> &ProviderCapabilities {
        &self.caps
    }

    async fn search(
        &self,
        query: &str,
        content_type: Option<ContentType>,
    ) -> Result<Vec<ProviderCandidate>, ProviderError> {
        if content_type.is_some_and(|ct| ct != ContentType::WebNovel) {
            return Ok(Vec::new());
        }
        let data: response::SearchResponse =
            self.client.get_json("/api/search", &[("q", query)]).await?;
        Ok(data.data.iter().filter_map(normalize::candidate).collect())
    }

    async fn get_details(&self, provider_id: &str) -> Result<ProviderMedia, ProviderError> {
        let html = self
            .client
            .get_html(&format!("/en/novel/{provider_id}"))
            .await?;
        let next_data =
            response::extract_next_data(&html).ok_or_else(|| ProviderError::InvalidResponse {
                provider: PROVIDER_ID.to_string(),
                message: "details page carried no __NEXT_DATA__ payload".to_string(),
            })?;
        normalize::media(&next_data, provider_id).ok_or_else(|| ProviderError::NotFound {
            provider: PROVIDER_ID.to_string(),
        })
    }

    async fn get_nodes(&self, provider_id: &str) -> Result<Vec<ProviderNode>, ProviderError> {
        let data: response::ChaptersResponse = self
            .client
            .get_json(
                &format!("/api/chapters/{}", Self::numeric_id(provider_id)),
                &[],
            )
            .await?;
        Ok(normalize::nodes(&data.chapters))
    }

    async fn get_external_ids(&self, _provider_id: &str) -> Result<Vec<ExternalId>, ProviderError> {
        // No cross-provider ids are exposed by the source today.
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::application::providers::coordinator::ProviderCoordinator;
    use crate::domain::enums::{MediaStatus, NodeKind, PersonRole};
    use crate::infrastructure::providers::test_support::fixture;

    fn provider_with(server: &MockServer) -> WtrLabProvider {
        WtrLabProvider::new(client::WtrLabClient::with_endpoint(
            reqwest::Client::new(),
            server.uri(),
        ))
    }

    #[tokio::test]
    async fn search_parses_rows_into_candidates() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/search"))
            .and(query_param("q", "emperor"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(fixture("wtrlab", "search.json")),
            )
            .mount(&server)
            .await;

        let hits = provider_with(&server)
            .search("emperor", None)
            .await
            .unwrap();
        assert_eq!(hits.len(), 1);
        let hit = &hits[0];
        assert_eq!(hit.provider_id, "93272/emperor-han-tian");
        assert_eq!(hit.title, "Emperor Han Tian");
        assert_eq!(hit.content_type, ContentType::WebNovel);
        assert!(hit
            .cover_url
            .as_deref()
            .unwrap()
            .starts_with("https://img.wtr-lab.com/"));
        let synopsis = hit.synopsis.as_deref().unwrap();
        assert!(synopsis.contains("immortal emperor"));
        assert!(!synopsis.contains('<'), "html must be stripped");
    }

    #[tokio::test]
    async fn search_skips_other_domains_without_requests() {
        let server = MockServer::start().await;
        let provider = provider_with(&server);
        let hits = provider
            .search("x", Some(ContentType::Anime))
            .await
            .unwrap();
        assert!(hits.is_empty());
    }

    #[tokio::test]
    async fn details_parse_the_next_data_payload() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/en/novel/93272/emperor-han-tian"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(fixture("wtrlab", "details.html")),
            )
            .mount(&server)
            .await;

        let media = provider_with(&server)
            .get_details("93272/emperor-han-tian")
            .await
            .unwrap();

        assert_eq!(media.provider_id, "93272/emperor-han-tian");
        assert_eq!(media.title_main, "Emperor Han Tian");
        assert_eq!(media.content_type, ContentType::WebNovel);
        assert_eq!(media.pub_status, MediaStatus::Completed);
        assert_eq!(media.ch_count, Some(965));
        assert!(media
            .title_original
            .as_deref()
            .is_some_and(|t| !t.is_empty()));
        assert!(media.alt_titles.contains(&"Han Tian Emperor".to_string()));
        assert!(media.genres.contains(&"Xianxia".to_string()));
        assert!(media.tags.contains(&"Cultivation".to_string()));
        assert!(media
            .people
            .iter()
            .any(|p| p.role == PersonRole::Author && p.name == "Test Author"));
        assert_eq!(
            media.url.as_deref(),
            Some("https://wtr-lab.com/en/novel/93272/emperor-han-tian")
        );
    }

    #[tokio::test]
    async fn nodes_are_ordered_chapters_preferring_translations() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/chapters/93272"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(fixture("wtrlab", "chapters.json")),
            )
            .mount(&server)
            .await;

        let nodes = provider_with(&server)
            .get_nodes("93272/emperor-han-tian")
            .await
            .unwrap();

        assert_eq!(nodes.len(), 3);
        // Feed arrives unordered; output must be sorted by order.
        assert_eq!(nodes[0].number.as_deref(), Some("1"));
        assert_eq!(nodes[2].number.as_deref(), Some("3"));
        assert_eq!(nodes[0].position, 1);
        // Empty EN title falls back to the raw name.
        assert_eq!(nodes[0].title.as_deref(), Some("?1? ???????"));
        assert_eq!(nodes[1].title.as_deref(), Some("Chapter 2 The Return"));
        for node in &nodes {
            assert_eq!(node.kind, NodeKind::Chapter);
        }
    }

    #[tokio::test]
    async fn challenge_pages_are_non_retryable() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/search"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("<html>Just a moment...<script>cf-turnstile</script></html>"),
            )
            .mount(&server)
            .await;

        let err = provider_with(&server).search("x", None).await.unwrap_err();
        assert!(matches!(err, ProviderError::InvalidResponse { .. }));
        assert!(!err.is_retryable());
    }

    #[tokio::test]
    async fn works_under_the_coordinator() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/search"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(fixture("wtrlab", "search.json")),
            )
            .mount(&server)
            .await;

        let entry = (
            wtrlab_config(),
            Arc::new(provider_with(&server)) as Arc<dyn Provider>,
        );
        let coordinator = ProviderCoordinator::new(vec![entry]).unwrap();
        let outcome = coordinator
            .search_all("emperor", Some(ContentType::WebNovel), &coordinator.token())
            .await;
        assert_eq!(outcome.hits.len(), 1);
        assert!(outcome.hits.iter().all(|h| h.provider == "wtrlab"));
        assert!(outcome.failures.is_empty());
    }
}
