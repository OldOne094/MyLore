//! AniList adapter (MISSION-054, API_PROVIDERS §1).
//!
//! The primary anime/manga provider: free public GraphQL at
//! `graphql.anilist.co` (no key, ~90 req/min). This adapter is a pure
//! normalizer — it maps AniList JSON into the unified domain types and typed
//! `ProviderError`s; all policy (rate limit, timeout, retry/backoff, cancel)
//! is applied by the `ProviderCoordinator`. Light novels are indexed under the
//! `MANGA` type with `NOVEL` format (`content_type` → `Novel`); web novels are
//! generally not indexed by AniList.

pub mod client;
mod graphql;
mod normalize;
mod response;

pub use client::{AniListClient, APP_USER_AGENT};

use async_trait::async_trait;
use serde_json::json;

use crate::application::providers::config::ProviderConfig;
use crate::domain::enums::ContentType;
use crate::domain::provider::capabilities::{AuthKind, ProviderCapabilities};
use crate::domain::provider::error::ProviderError;
use crate::domain::provider::types::{
    ProviderCandidate, ProviderMedia, ProviderNode, ProviderRelation,
};
use crate::domain::provider::Provider;
use crate::domain::value_objects::ExternalId;

pub const PROVIDER_ID: &str = "anilist";
pub const ENDPOINT: &str = "https://graphql.anilist.co";
/// ~90 req/min documented per IP; schedule at 1.5 rps to stay under it.
pub const REQUESTS_PER_SEC: f64 = 1.5;

/// The config the coordinator registers AniList with. It serves the anime and
/// manga domains (incl. manhwa/manhua and light novels); books/TV/movies are
/// routed to other providers.
pub fn anilist_config() -> ProviderConfig {
    ProviderConfig::new(PROVIDER_ID)
        .with_requests_per_sec(REQUESTS_PER_SEC)
        .with_content_types(vec![
            ContentType::Anime,
            ContentType::Manga,
            ContentType::Manhwa,
            ContentType::Manhua,
            ContentType::Novel,
        ])
}

pub struct AniListProvider {
    client: AniListClient,
    caps: ProviderCapabilities,
}

impl AniListProvider {
    pub fn new(client: AniListClient) -> Self {
        Self {
            client,
            caps: ProviderCapabilities {
                search: true,
                details: true,
                nodes: true,
                related: true,
                reviews: false,
                images: true,
                seasonal: false,
                auth: AuthKind::None,
            },
        }
    }

    /// Map a domain content type to the AniList `MediaType` filter. Domains
    /// AniList doesn't index map to `None` (no filter).
    fn anilist_type(content_type: Option<ContentType>) -> Option<&'static str> {
        match content_type {
            Some(ContentType::Anime) => Some("ANIME"),
            Some(
                ContentType::Manga | ContentType::Manhwa | ContentType::Manhua | ContentType::Novel,
            ) => Some("MANGA"),
            _ => None,
        }
    }

    async fn fetch_media(&self, provider_id: &str) -> Result<response::MediaFull, ProviderError> {
        let id: i64 = provider_id
            .parse()
            .map_err(|_| ProviderError::InvalidResponse {
                provider: PROVIDER_ID.to_string(),
                message: format!("expected a numeric AniList id, got {provider_id:?}"),
            })?;
        let data: response::DetailsData = self
            .client
            .graphql(graphql::DETAILS_QUERY, json!({ "id": id }))
            .await?;
        data.media.ok_or_else(|| ProviderError::NotFound {
            provider: PROVIDER_ID.to_string(),
        })
    }
}

#[async_trait]
impl Provider for AniListProvider {
    fn id(&self) -> String {
        PROVIDER_ID.to_string()
    }

    fn name(&self) -> &str {
        "AniList"
    }

    fn capabilities(&self) -> &ProviderCapabilities {
        &self.caps
    }

    async fn search(
        &self,
        query: &str,
        content_type: Option<ContentType>,
    ) -> Result<Vec<ProviderCandidate>, ProviderError> {
        let variables = json!({
            "q": query,
            "type": Self::anilist_type(content_type),
            "page": 1,
            "perPage": 20,
        });
        let data: response::SearchData = self
            .client
            .graphql(graphql::SEARCH_QUERY, variables)
            .await?;
        Ok(data.page.media.iter().map(normalize::candidate).collect())
    }

    async fn get_details(&self, provider_id: &str) -> Result<ProviderMedia, ProviderError> {
        Ok(normalize::media(&self.fetch_media(provider_id).await?))
    }

    async fn get_nodes(&self, provider_id: &str) -> Result<Vec<ProviderNode>, ProviderError> {
        Ok(normalize::nodes(&self.fetch_media(provider_id).await?))
    }

    async fn get_related(&self, provider_id: &str) -> Result<Vec<ProviderRelation>, ProviderError> {
        Ok(normalize::relations(&self.fetch_media(provider_id).await?))
    }

    async fn get_external_ids(&self, provider_id: &str) -> Result<Vec<ExternalId>, ProviderError> {
        let media = self.fetch_media(provider_id).await?;
        Ok(normalize::external_ids(
            media.external_links.as_deref().unwrap_or(&[]),
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::application::providers::coordinator::ProviderCoordinator;
    use crate::infrastructure::providers::test_support::fixture;

    async fn provider_with_fixture(server: &MockServer, name: &str) -> AniListProvider {
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_string(fixture("anilist", name)))
            .mount(server)
            .await;
        let client = AniListClient::with_endpoint(reqwest::Client::new(), server.uri());
        AniListProvider::new(client)
    }

    #[tokio::test]
    async fn search_returns_normalized_candidates() {
        let server = MockServer::start().await;
        let provider = provider_with_fixture(&server, "search_anime.json").await;
        let hits = provider
            .search("bebop", Some(ContentType::Anime))
            .await
            .unwrap();
        assert_eq!(hits.len(), 2);
        assert!(hits.iter().all(|c| c.content_type == ContentType::Anime));
        assert_eq!(hits[0].provider_id, "1");
        assert_eq!(hits[0].title, "Cowboy Bebop");
        assert_eq!(hits[0].release_year, Some(1998));
    }

    #[tokio::test]
    async fn get_details_returns_normalized_media() {
        let server = MockServer::start().await;
        let provider = provider_with_fixture(&server, "details_anime.json").await;
        let media = provider.get_details("1").await.unwrap();
        assert_eq!(media.provider_id, "1");
        assert_eq!(media.title_main, "Cowboy Bebop");
        assert_eq!(media.content_type, ContentType::Anime);
        assert_eq!(media.ep_count, Some(26));
        assert!(!media.external_ids.is_empty());
    }

    #[tokio::test]
    async fn get_details_404s_on_null_media() {
        let server = MockServer::start().await;
        let client = AniListClient::with_endpoint(reqwest::Client::new(), server.uri());
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"data":{"Media":null}}"#))
            .mount(&server)
            .await;
        let provider = AniListProvider::new(client);
        let err = provider.get_details("999999").await.unwrap_err();
        assert!(matches!(err, ProviderError::NotFound { .. }));
    }

    #[tokio::test]
    async fn works_under_the_coordinator() {
        let server = MockServer::start().await;
        let provider = provider_with_fixture(&server, "search_anime.json").await;
        let entry = (anilist_config(), Arc::new(provider) as Arc<dyn Provider>);
        let coordinator = ProviderCoordinator::new(vec![entry]).unwrap();
        let outcome = coordinator
            .search_all("bebop", Some(ContentType::Anime), &coordinator.token())
            .await;
        assert_eq!(outcome.hits.len(), 2);
        assert!(outcome.hits.iter().all(|h| h.provider == "anilist"));
        assert!(outcome.failures.is_empty());
    }
}
