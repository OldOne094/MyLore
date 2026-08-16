//! Hardcover adapter (MISSION-064, API_PROVIDERS §12).
//!
//! The optional third book provider: free GraphQL at `api.hardcover.app`
//! (Hasura, Bearer-token auth), positioned as a Goodreads-API alternative —
//! strong indie/editions polish and explicit light/web-novel categories. The
//! "young/indie, schema churn" risk (API_PROVIDERS) is accepted: the adapter
//! only reads documented fields and parses the Typesense search blob
//! defensively. Provider ids are numeric book ids.
//!
//! Like all adapters, this is a pure normalizer; all policy (rate limit,
//! timeout, retry/backoff, cancel) is applied by the `ProviderCoordinator`.

pub mod client;
mod graphql;
mod normalize;
mod response;

pub use client::{HardcoverClient, APP_USER_AGENT};

use async_trait::async_trait;
use serde_json::json;

use crate::application::providers::config::ProviderConfig;
use crate::domain::enums::ContentType;
use crate::domain::provider::capabilities::{AuthKind, ProviderCapabilities};
use crate::domain::provider::error::ProviderError;
use crate::domain::provider::types::{ProviderCandidate, ProviderMedia};
use crate::domain::provider::Provider;
use crate::domain::value_objects::ExternalId;

pub const PROVIDER_ID: &str = "hardcover";
pub const ENDPOINT: &str = "https://api.hardcover.app/v1/graphql";
/// Rate limits are not published (flagged in API_PROVIDERS) → self-throttle
/// conservatively at ~1 rps.
pub const REQUESTS_PER_SEC: f64 = 1.0;

/// The config the coordinator registers Hardcover with. Books + light/web
/// novels (details re-derive the type via `book_category_id`).
pub fn hardcover_config() -> ProviderConfig {
    ProviderConfig::new(PROVIDER_ID)
        .with_requests_per_sec(REQUESTS_PER_SEC)
        .with_content_types(vec![
            ContentType::Book,
            ContentType::Novel,
            ContentType::WebNovel,
        ])
}

pub struct HardcoverProvider {
    client: HardcoverClient,
    caps: ProviderCapabilities,
}

impl HardcoverProvider {
    pub fn new(client: HardcoverClient) -> Self {
        Self {
            client,
            caps: ProviderCapabilities {
                search: true,
                details: true,
                nodes: false,   // books/editions — no chapter tree
                related: false, // no relation edges exposed
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
            message: format!("expected a numeric Hardcover book id, got {provider_id:?}"),
        }
    }

    /// Validate + normalize a book id (numeric; `a/b`, `-1`, blanks rejected).
    fn parse_book_id(&self, provider_id: &str) -> Result<i64, ProviderError> {
        provider_id
            .parse::<i64>()
            .map_err(|_| Self::invalid_id(provider_id))
    }

    async fn fetch_book(&self, provider_id: &str) -> Result<response::Book, ProviderError> {
        let id = self.parse_book_id(provider_id)?;
        let data: response::BooksData = self
            .client
            .graphql(graphql::DETAILS_QUERY, json!({ "id": id }))
            .await?;
        data.books
            .into_iter()
            .next()
            .ok_or_else(|| Self::invalid_id(provider_id))
    }
}

#[async_trait]
impl Provider for HardcoverProvider {
    fn id(&self) -> String {
        PROVIDER_ID.to_string()
    }

    fn name(&self) -> &str {
        "Hardcover"
    }

    fn capabilities(&self) -> &ProviderCapabilities {
        &self.caps
    }

    async fn search(
        &self,
        query: &str,
        content_type: Option<ContentType>,
    ) -> Result<Vec<ProviderCandidate>, ProviderError> {
        if content_type.is_some_and(|ct| {
            !matches!(
                ct,
                ContentType::Book | ContentType::Novel | ContentType::WebNovel
            )
        }) {
            return Ok(Vec::new());
        }
        let data: response::SearchPayload = self
            .client
            .graphql(
                graphql::SEARCH_QUERY,
                json!({ "query": query, "per_page": 20, "page": 1 }),
            )
            .await?;
        let candidates = data
            .search
            .results
            .iter()
            .filter_map(|value| serde_json::from_value::<response::SearchBook>(value.clone()).ok())
            .filter_map(|row| normalize::candidate(&row));
        Ok(candidates.collect())
    }

    async fn get_details(&self, provider_id: &str) -> Result<ProviderMedia, ProviderError> {
        let book = self.fetch_book(provider_id).await?;
        normalize::media(&book).ok_or_else(|| Self::invalid_id(provider_id))
    }

    // get_nodes, get_related stay on the trait defaults (→ `Unsupported`).

    async fn get_external_ids(&self, provider_id: &str) -> Result<Vec<ExternalId>, ProviderError> {
        let book = self.fetch_book(provider_id).await?;
        Ok(normalize::external_ids(&book))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use wiremock::matchers::{body_partial_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::application::providers::coordinator::ProviderCoordinator;
    use crate::infrastructure::providers::test_support::hardcover_fixture;

    const DUNE: &str = "3342";

    fn provider_with(server: &MockServer) -> HardcoverProvider {
        HardcoverProvider::new(HardcoverClient::with_endpoint(
            reqwest::Client::new(),
            server.uri(),
        ))
    }

    async fn mount(server: &MockServer, query: &str, fixture: &str) {
        Mock::given(method("POST"))
            .and(path("/"))
            .and(body_partial_json(json!({ "query": query })))
            .respond_with(ResponseTemplate::new(200).set_body_string(hardcover_fixture(fixture)))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn search_returns_candidates() {
        let server = MockServer::start().await;
        mount(&server, graphql::SEARCH_QUERY, "search_books.json").await;
        let provider = provider_with(&server);
        let hits = provider.search("dune", None).await.unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].provider_id, DUNE);
        assert_eq!(hits[0].content_type, ContentType::Book);
    }

    #[tokio::test]
    async fn search_sends_query_variables() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/"))
            .and(body_partial_json(json!({ "query": graphql::SEARCH_QUERY })))
            .and(body_partial_json(
                json!({ "variables": { "query": "dune" } }),
            ))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(hardcover_fixture("search_books.json")),
            )
            .mount(&server)
            .await;
        let provider = provider_with(&server);
        let hits = provider.search("dune", None).await.unwrap();
        assert_eq!(hits.len(), 2);
    }

    #[tokio::test]
    async fn search_serves_book_domains_and_short_circuits_others() {
        let server = MockServer::start().await;
        mount(&server, graphql::SEARCH_QUERY, "search_books.json").await;
        let provider = provider_with(&server);
        let hits = provider
            .search("dune", Some(ContentType::Book))
            .await
            .unwrap();
        assert_eq!(hits.len(), 2);
        let hits = provider
            .search("dune", Some(ContentType::Novel))
            .await
            .unwrap();
        assert_eq!(hits.len(), 2, "light novels are routed here too");
        let hits = provider
            .search("cowboy", Some(ContentType::Anime))
            .await
            .unwrap();
        assert!(hits.is_empty());
    }

    #[tokio::test]
    async fn get_details_normalizes() {
        let server = MockServer::start().await;
        mount(&server, graphql::DETAILS_QUERY, "book_details.json").await;
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
    async fn get_details_rejects_invalid_ids() {
        let server = MockServer::start().await;
        let provider = provider_with(&server);
        let err = provider.get_details("a/b").await.unwrap_err();
        assert!(matches!(err, ProviderError::InvalidResponse { .. }));
        let err = provider.get_details("").await.unwrap_err();
        assert!(matches!(err, ProviderError::InvalidResponse { .. }));
    }

    #[tokio::test]
    async fn get_details_rejects_empty_book_payload() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"data":{"books":[]},"errors":null}"#),
            )
            .mount(&server)
            .await;
        let provider = provider_with(&server);
        let err = provider.get_details(DUNE).await.unwrap_err();
        assert!(matches!(err, ProviderError::InvalidResponse { .. }));
    }

    #[tokio::test]
    async fn get_external_ids_from_edition_isbns() {
        let server = MockServer::start().await;
        mount(&server, graphql::DETAILS_QUERY, "book_details.json").await;
        let provider = provider_with(&server);
        let ids = provider.get_external_ids(DUNE).await.unwrap();
        assert!(ids.iter().any(|e| e.provider().as_str() == "isbn10"));
        assert!(ids.iter().any(|e| e.provider().as_str() == "isbn13"));
    }

    #[tokio::test]
    async fn works_under_the_coordinator() {
        let server = MockServer::start().await;
        mount(&server, graphql::SEARCH_QUERY, "search_books.json").await;
        let provider = provider_with(&server);
        let entry = (hardcover_config(), Arc::new(provider) as Arc<dyn Provider>);
        let coordinator = ProviderCoordinator::new(vec![entry]).unwrap();
        let outcome = coordinator
            .search_all("dune", Some(ContentType::Book), &coordinator.token())
            .await;
        assert_eq!(outcome.hits.len(), 2);
        assert!(outcome.hits.iter().all(|h| h.provider == "hardcover"));
        assert!(outcome.failures.is_empty());
    }
}
