//! OpenLibrary adapter (MISSION-057, API_PROVIDERS §7).
//!
//! The primary book provider: free keyless REST at `openlibrary.org`, ~1 req/s.
//! This adapter is a pure normalizer — it maps OpenLibrary JSON into the unified
//! domain types and typed `ProviderError`s; all policy (rate limit, timeout,
//! retry/backoff, cancel) is applied by the `ProviderCoordinator`. Provider ids
//! are bare work keys (`OL89650W`, the `/works/…` suffix). Books have no node
//! tree and OpenLibrary exposes no sequel/prequel edges, so `nodes`/`related`
//! are off; external ids (ISBNs/LCCN/OCLC) come from the first edition and feed
//! the dedup mission.

pub mod client;
mod normalize;
mod response;

pub use client::{OpenLibraryClient, APP_USER_AGENT};

use async_trait::async_trait;

use crate::application::providers::config::ProviderConfig;
use crate::domain::enums::ContentType;
use crate::domain::provider::capabilities::{AuthKind, ProviderCapabilities};
use crate::domain::provider::error::ProviderError;
use crate::domain::provider::types::{ProviderCandidate, ProviderMedia};
use crate::domain::provider::Provider;
use crate::domain::value_objects::ExternalId;

pub const PROVIDER_ID: &str = "openlibrary";
pub const ENDPOINT: &str = "https://openlibrary.org";
/// 1 req/s documented default (3 with a contact UA; we schedule conservatively).
pub const REQUESTS_PER_SEC: f64 = 1.0;
/// Cap author-resolution calls per work (works rarely exceed one or two authors).
const MAX_AUTHORS: usize = 5;

/// The config the coordinator registers OpenLibrary with. Books only.
pub fn openlibrary_config() -> ProviderConfig {
    ProviderConfig::new(PROVIDER_ID)
        .with_requests_per_sec(REQUESTS_PER_SEC)
        .with_content_types(vec![ContentType::Book])
}

pub struct OpenLibraryProvider {
    client: OpenLibraryClient,
    caps: ProviderCapabilities,
}

impl OpenLibraryProvider {
    pub fn new(client: OpenLibraryClient) -> Self {
        Self {
            client,
            caps: ProviderCapabilities {
                search: true,
                details: true,
                nodes: false,   // books have no chapter tree
                related: false, // no sequel/prequel edges on OpenLibrary
                reviews: false,
                images: true,
                seasonal: false,
                auth: AuthKind::None,
            },
        }
    }

    fn invalid_id(provider_id: &str) -> ProviderError {
        ProviderError::InvalidResponse {
            provider: PROVIDER_ID.to_string(),
            message: format!(
                "expected a bare OpenLibrary work key like `OL89650W`, got {provider_id:?}"
            ),
        }
    }

    async fn fetch_work(&self, provider_id: &str) -> Result<response::WorkResponse, ProviderError> {
        let id = normalize::work_id(provider_id);
        if !id.starts_with("OL") {
            return Err(Self::invalid_id(provider_id));
        }
        self.client.get(&format!("/works/{id}.json"), &[]).await
    }

    /// Resolve the work's author keys to display names (one call per author).
    async fn fetch_authors(
        &self,
        work: &response::WorkResponse,
    ) -> Result<Vec<String>, ProviderError> {
        let mut names = Vec::new();
        for author in work
            .authors
            .as_deref()
            .unwrap_or_default()
            .iter()
            .take(MAX_AUTHORS)
        {
            let Some(key) = author.author.as_ref().and_then(|a| a.key.as_deref()) else {
                continue;
            };
            let id = normalize::work_id(key);
            if id.is_empty() || !id.starts_with("OL") {
                continue;
            }
            let resolved: response::AuthorResponse =
                self.client.get(&format!("/authors/{id}.json"), &[]).await?;
            if let Some(name) = resolved.name {
                names.push(name);
            }
        }
        Ok(names)
    }
}

#[async_trait]
impl Provider for OpenLibraryProvider {
    fn id(&self) -> String {
        PROVIDER_ID.to_string()
    }

    fn name(&self) -> &str {
        "OpenLibrary"
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
        let data: response::SearchResponse = self
            .client
            .get(
                "/search.json",
                &[
                    ("q", query),
                    ("limit", "20"),
                    (
                        "fields",
                        "key,title,author_name,first_publish_year,cover_i,subject",
                    ),
                ],
            )
            .await?;
        Ok(data.docs.iter().filter_map(normalize::candidate).collect())
    }

    async fn get_details(&self, provider_id: &str) -> Result<ProviderMedia, ProviderError> {
        let work = self.fetch_work(provider_id).await?;
        let names = self.fetch_authors(&work).await?;
        Ok(normalize::media(&work, normalize::authors(&names)))
    }

    // get_nodes, get_related stay on the trait defaults (→ `Unsupported`).
    // No banner image, no start/end dates, no ratings — all left `None`.

    async fn get_external_ids(&self, provider_id: &str) -> Result<Vec<ExternalId>, ProviderError> {
        let id = normalize::work_id(provider_id);
        let data: response::EditionsResponse = self
            .client
            .get(
                &format!("/works/{id}/editions.json"),
                &[
                    ("limit", "1"),
                    ("fields", "title,isbn_10,isbn_13,lccn,oclc_numbers"),
                ],
            )
            .await?;
        let Some(edition) = data.docs.first() else {
            return Ok(Vec::new());
        };
        Ok(normalize::external_ids(edition))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::application::providers::coordinator::ProviderCoordinator;
    use crate::infrastructure::providers::test_support::fixture;

    const DUNE: &str = "OL89650W";

    fn provider_with(server: &MockServer) -> OpenLibraryProvider {
        OpenLibraryProvider::new(OpenLibraryClient::with_endpoint(
            reqwest::Client::new(),
            server.uri(),
        ))
    }

    async fn mount(server: &MockServer, route: &str, name: &str) {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(ResponseTemplate::new(200).set_body_string(fixture("openlibrary", name)))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn search_returns_candidates() {
        let server = MockServer::start().await;
        mount(&server, "/search.json", "search_books.json").await;
        let provider = provider_with(&server);
        let hits = provider.search("dune", None).await.unwrap();
        assert_eq!(hits.len(), 1);
        let hit = &hits[0];
        assert_eq!(hit.provider_id, DUNE);
        assert_eq!(hit.content_type, ContentType::Book);
        assert_eq!(hit.release_year, Some(1965));
        assert!(hit.cover_url.is_some());
    }

    #[tokio::test]
    async fn search_sends_query_and_limits() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/search.json"))
            .and(query_param("q", "dune"))
            .and(query_param("limit", "20"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(fixture("openlibrary", "search_books.json")),
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
    async fn get_details_resolves_authors() {
        let server = MockServer::start().await;
        mount(&server, &format!("/works/{DUNE}.json"), "work.json").await;
        mount(&server, "/authors/OL26523A.json", "author.json").await;
        let provider = provider_with(&server);
        let media = provider.get_details(DUNE).await.unwrap();
        assert_eq!(media.provider_id, DUNE);
        assert_eq!(media.title_main, "Dune");
        assert_eq!(media.content_type, ContentType::Book);
        assert_eq!(media.release_year, Some(1965));
        assert_eq!(media.people.len(), 1);
        assert_eq!(media.people[0].name, "Frank Herbert");
        assert!(media.genres.iter().any(|g| g == "Science fiction"));
    }

    #[tokio::test]
    async fn get_details_maps_404_to_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/works/OLX.json"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let provider = provider_with(&server);
        let err = provider.get_details("OLX").await.unwrap_err();
        assert!(matches!(err, ProviderError::NotFound { .. }));
    }

    #[tokio::test]
    async fn get_details_rejects_non_work_ids() {
        let server = MockServer::start().await;
        let provider = provider_with(&server);
        let err = provider.get_details("tv-1").await.unwrap_err();
        assert!(matches!(err, ProviderError::InvalidResponse { .. }));
    }

    #[tokio::test]
    async fn get_external_ids_from_first_edition() {
        let server = MockServer::start().await;
        mount(
            &server,
            &format!("/works/{DUNE}/editions.json"),
            "editions.json",
        )
        .await;
        let provider = provider_with(&server);
        let ids = provider.get_external_ids(DUNE).await.unwrap();
        assert!(ids.iter().any(|e| e.provider().as_str() == "isbn10"));
        assert!(ids.iter().any(|e| e.provider().as_str() == "isbn13"));
        assert!(ids.iter().any(|e| e.provider().as_str() == "lccn"));
        assert!(ids.iter().any(|e| e.provider().as_str() == "oclc"));
    }

    #[tokio::test]
    async fn works_under_the_coordinator() {
        let server = MockServer::start().await;
        mount(&server, "/search.json", "search_books.json").await;
        let provider = provider_with(&server);
        let entry = (
            openlibrary_config(),
            Arc::new(provider) as Arc<dyn Provider>,
        );
        let coordinator = ProviderCoordinator::new(vec![entry]).unwrap();
        let outcome = coordinator
            .search_all("dune", Some(ContentType::Book), &coordinator.token())
            .await;
        assert_eq!(outcome.hits.len(), 1);
        assert!(outcome.hits.iter().all(|h| h.provider == "openlibrary"));
        assert!(outcome.failures.is_empty());
    }
}
