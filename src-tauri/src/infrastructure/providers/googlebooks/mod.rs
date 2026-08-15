//! Google Books adapter (MISSION-058, API_PROVIDERS §8).
//!
//! The secondary book provider: keyed REST at `www.googleapis.com/books/v1`
//! (~100 queries/min per project), strong for non-English and preview data. It
//! answers the same `Book` searches as OpenLibrary, so the coordinator's
//! parallel fan-out gives a live fallback. Provider ids are Google volume ids.
//!
//! Like all adapters, this is a pure normalizer; all policy (rate limit,
//! timeout, retry/backoff, cancel) is applied by the `ProviderCoordinator`.

pub mod client;
mod normalize;
mod response;

pub use client::{GoogleBooksClient, APP_USER_AGENT};

use async_trait::async_trait;

use crate::application::providers::config::ProviderConfig;
use crate::domain::enums::ContentType;
use crate::domain::provider::capabilities::{AuthKind, ProviderCapabilities};
use crate::domain::provider::error::ProviderError;
use crate::domain::provider::types::{ProviderCandidate, ProviderMedia};
use crate::domain::provider::Provider;
use crate::domain::value_objects::ExternalId;

pub const PROVIDER_ID: &str = "googlebooks";
pub const ENDPOINT: &str = "https://www.googleapis.com/books/v1";
/// ~100 queries/min per-project quota → schedule at 1 rps to stay well under.
pub const REQUESTS_PER_SEC: f64 = 1.0;

/// The config the coordinator registers Google Books with. Books only.
pub fn googlebooks_config() -> ProviderConfig {
    ProviderConfig::new(PROVIDER_ID)
        .with_requests_per_sec(REQUESTS_PER_SEC)
        .with_content_types(vec![ContentType::Book])
}

pub struct GoogleBooksProvider {
    client: GoogleBooksClient,
    caps: ProviderCapabilities,
}

impl GoogleBooksProvider {
    pub fn new(client: GoogleBooksClient) -> Self {
        Self {
            client,
            caps: ProviderCapabilities {
                search: true,
                details: true,
                nodes: false,   // books have no chapter tree
                related: false, // no relation edges on Google Books
                reviews: false,
                images: true,
                seasonal: false,
                auth: AuthKind::Key,
            },
        }
    }

    fn invalid_id(provider_id: &str) -> ProviderError {
        ProviderError::InvalidResponse {
            provider: PROVIDER_ID.to_string(),
            message: format!("expected a Google volume id, got {provider_id:?}"),
        }
    }

    async fn fetch_volume(&self, provider_id: &str) -> Result<response::Volume, ProviderError> {
        if provider_id.is_empty() || provider_id.contains('/') {
            return Err(Self::invalid_id(provider_id));
        }
        self.client
            .get(&format!("/volumes/{provider_id}"), &[])
            .await
    }
}

#[async_trait]
impl Provider for GoogleBooksProvider {
    fn id(&self) -> String {
        PROVIDER_ID.to_string()
    }

    fn name(&self) -> &str {
        "Google Books"
    }

    fn capabilities(&self) -> &ProviderCapabilities {
        &self.caps
    }

    async fn search(
        &self,
        query: &str,
        content_type: Option<ContentType>,
    ) -> Result<Vec<ProviderCandidate>, ProviderError> {
        if content_type.is_some_and(|ct| ct != ContentType::Book) {
            return Ok(Vec::new());
        }
        let data: response::VolumesResponse = self
            .client
            .get(
                "/volumes",
                &[("q", query), ("maxResults", "20"), ("printType", "books")],
            )
            .await?;
        Ok(data.items.iter().filter_map(normalize::candidate).collect())
    }

    async fn get_details(&self, provider_id: &str) -> Result<ProviderMedia, ProviderError> {
        let volume = self.fetch_volume(provider_id).await?;
        normalize::media(&volume).ok_or_else(|| Self::invalid_id(provider_id))
    }

    // get_nodes, get_related stay on the trait defaults (→ `Unsupported`).

    async fn get_external_ids(&self, provider_id: &str) -> Result<Vec<ExternalId>, ProviderError> {
        let volume = self.fetch_volume(provider_id).await?;
        let Some(info) = volume.volume_info.as_ref() else {
            return Ok(Vec::new());
        };
        Ok(normalize::external_ids(info))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::application::providers::coordinator::ProviderCoordinator;
    use crate::infrastructure::providers::test_support::googlebooks_fixture;

    const DUNE: &str = "l4YzAwAAQBAJ";

    fn provider_with(server: &MockServer) -> GoogleBooksProvider {
        GoogleBooksProvider::new(GoogleBooksClient::with_endpoint(
            reqwest::Client::new(),
            server.uri(),
        ))
    }

    async fn mount(server: &MockServer, route: &str, fixture: &str) {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(ResponseTemplate::new(200).set_body_string(googlebooks_fixture(fixture)))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn search_returns_candidates() {
        let server = MockServer::start().await;
        mount(&server, "/volumes", "search_volumes.json").await;
        let provider = provider_with(&server);
        let hits = provider.search("dune", None).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].provider_id, DUNE);
        assert_eq!(hits[0].content_type, ContentType::Book);
    }

    #[tokio::test]
    async fn search_sends_query_and_limits() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/volumes"))
            .and(query_param("q", "dune"))
            .and(query_param("maxResults", "20"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(googlebooks_fixture("search_volumes.json")),
            )
            .mount(&server)
            .await;
        let provider = provider_with(&server);
        let hits = provider.search("dune", None).await.unwrap();
        assert_eq!(hits.len(), 1);
    }

    #[tokio::test]
    async fn search_for_non_book_domains_is_empty_without_requests() {
        let server = MockServer::start().await;
        let provider = provider_with(&server);
        let hits = provider
            .search("cowboy", Some(ContentType::Anime))
            .await
            .unwrap();
        assert!(hits.is_empty());
    }

    #[tokio::test]
    async fn get_details_normalizes() {
        let server = MockServer::start().await;
        mount(&server, &format!("/volumes/{DUNE}"), "volume_details.json").await;
        let provider = provider_with(&server);
        let media = provider.get_details(DUNE).await.unwrap();
        assert_eq!(media.provider_id, DUNE);
        assert_eq!(media.title_main, "Dune");
        assert_eq!(media.content_type, ContentType::Book);
        assert_eq!(media.pages, Some(704));
        assert_eq!(media.people.len(), 1);
        assert!(media
            .external_ids
            .iter()
            .any(|e| e.provider().as_str() == "isbn13"));
    }

    #[tokio::test]
    async fn get_details_maps_404_to_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/volumes/nope"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let provider = provider_with(&server);
        let err = provider.get_details("nope").await.unwrap_err();
        assert!(matches!(err, ProviderError::NotFound { .. }));
    }

    #[tokio::test]
    async fn get_details_rejects_invalid_ids() {
        let server = MockServer::start().await;
        let provider = provider_with(&server);
        let err = provider.get_details("a/b").await.unwrap_err();
        assert!(matches!(err, ProviderError::InvalidResponse { .. }));
    }

    #[tokio::test]
    async fn get_external_ids_from_industry_identifiers() {
        let server = MockServer::start().await;
        mount(&server, &format!("/volumes/{DUNE}"), "volume_details.json").await;
        let provider = provider_with(&server);
        let ids = provider.get_external_ids(DUNE).await.unwrap();
        assert!(ids.iter().any(|e| e.provider().as_str() == "isbn10"));
        assert!(ids.iter().any(|e| e.provider().as_str() == "isbn13"));
    }

    #[tokio::test]
    async fn works_under_the_coordinator() {
        let server = MockServer::start().await;
        mount(&server, "/volumes", "search_volumes.json").await;
        let provider = provider_with(&server);
        let entry = (
            googlebooks_config(),
            Arc::new(provider) as Arc<dyn Provider>,
        );
        let coordinator = ProviderCoordinator::new(vec![entry]).unwrap();
        let outcome = coordinator
            .search_all("dune", Some(ContentType::Book), &coordinator.token())
            .await;
        assert_eq!(outcome.hits.len(), 1);
        assert!(outcome.hits.iter().all(|h| h.provider == "googlebooks"));
        assert!(outcome.failures.is_empty());
    }
}
