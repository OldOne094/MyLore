//! Jikan adapter (MISSION-058, API_PROVIDERS §2).
//!
//! The anime fallback: an unofficial, keyless mirror of MyAnimeList at
//! `api.jikan.moe/v4` (~3 req/s, 60 req/min). It answers `Anime`/`Movie`
//! searches the same way AniList does, so the coordinator's parallel fan-out
//! gives us a live fallback when AniList is down — one provider failing never
//! fails the search (MISSION-053). Provider ids are MAL ids (numeric) and every
//! title also carries a `mal` external id for dedup regardless of source.
//!
//! Like all adapters, this is a pure normalizer; all policy (rate limit,
//! timeout, retry/backoff, cancel) is applied by the `ProviderCoordinator`.

pub mod client;
mod normalize;
mod response;

pub use client::{JikanClient, APP_USER_AGENT};

use async_trait::async_trait;

use crate::application::providers::config::ProviderConfig;
use crate::domain::enums::ContentType;
use crate::domain::provider::capabilities::{AuthKind, ProviderCapabilities};
use crate::domain::provider::error::ProviderError;
use crate::domain::provider::types::{ProviderCandidate, ProviderMedia, ProviderNode};
use crate::domain::provider::Provider;
use crate::domain::value_objects::ExternalId;

pub const PROVIDER_ID: &str = "jikan";
pub const ENDPOINT: &str = "https://api.jikan.moe/v4";
/// ~3 req/s, 60 req/min community-documented → schedule at 2 rps, cache UIs.
pub const REQUESTS_PER_SEC: f64 = 2.0;

/// The config the coordinator registers Jikan with. Anime + anime films.
pub fn jikan_config() -> ProviderConfig {
    ProviderConfig::new(PROVIDER_ID)
        .with_requests_per_sec(REQUESTS_PER_SEC)
        .with_content_types(vec![ContentType::Anime, ContentType::Movie])
}

pub struct JikanProvider {
    client: JikanClient,
    caps: ProviderCapabilities,
}

impl JikanProvider {
    pub fn new(client: JikanClient) -> Self {
        Self {
            client,
            caps: ProviderCapabilities {
                search: true,
                details: true,
                nodes: true,
                related: false,
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
            message: format!("expected a numeric MAL anime id, got {provider_id:?}"),
        }
    }

    async fn fetch_anime(&self, provider_id: &str) -> Result<response::Anime, ProviderError> {
        if provider_id.is_empty() || !provider_id.chars().all(|c| c.is_ascii_digit()) {
            return Err(Self::invalid_id(provider_id));
        }
        let data: response::AnimeDetailResponse = self
            .client
            .get(&format!("/anime/{provider_id}"), &[])
            .await?;
        data.data.ok_or_else(|| ProviderError::NotFound {
            provider: PROVIDER_ID.to_string(),
        })
    }
}

#[async_trait]
impl Provider for JikanProvider {
    fn id(&self) -> String {
        PROVIDER_ID.to_string()
    }

    fn name(&self) -> &str {
        "Jikan (MyAnimeList)"
    }

    fn capabilities(&self) -> &ProviderCapabilities {
        &self.caps
    }

    async fn search(
        &self,
        query: &str,
        content_type: Option<ContentType>,
    ) -> Result<Vec<ProviderCandidate>, ProviderError> {
        if content_type.is_some_and(|ct| !matches!(ct, ContentType::Anime | ContentType::Movie)) {
            return Ok(Vec::new());
        }
        let data: response::AnimeSearchResponse = self
            .client
            .get(
                "/anime",
                &[
                    ("q", query),
                    ("limit", "20"),
                    ("order_by", "popularity"),
                    ("sort", "asc"),
                ],
            )
            .await?;
        Ok(data
            .data
            .iter()
            .filter_map(|anime| {
                let hit_type = normalize::content_type(anime.r#type.as_deref());
                if content_type.is_some_and(|wanted| wanted != hit_type) {
                    return None;
                }
                normalize::candidate(anime)
            })
            .collect())
    }

    async fn get_details(&self, provider_id: &str) -> Result<ProviderMedia, ProviderError> {
        let anime = self.fetch_anime(provider_id).await?;
        normalize::media(&anime).ok_or_else(|| Self::invalid_id(provider_id))
    }

    async fn get_nodes(&self, provider_id: &str) -> Result<Vec<ProviderNode>, ProviderError> {
        self.fetch_anime(provider_id).await?;
        let data: response::EpisodesResponse = self
            .client
            .get(
                &format!("/anime/{provider_id}/episodes"),
                &[("limit", "100"), ("page", "1")],
            )
            .await?;
        Ok(normalize::nodes(&data.data, provider_id))
    }

    // get_related stays on the trait default (→ `Unsupported`).

    async fn get_external_ids(&self, provider_id: &str) -> Result<Vec<ExternalId>, ProviderError> {
        // The MAL id is our provider id; always surface it for dedup.
        Ok(normalize::push_mal(Vec::new(), provider_id))
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

    const HUNTER: &str = "11061";

    fn provider_with(server: &MockServer) -> JikanProvider {
        JikanProvider::new(JikanClient::with_endpoint(
            reqwest::Client::new(),
            server.uri(),
        ))
    }

    async fn mount(server: &MockServer, route: &str, name: &str) {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(ResponseTemplate::new(200).set_body_string(fixture("jikan", name)))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn search_returns_candidates() {
        let server = MockServer::start().await;
        mount(&server, "/anime", "search_anime.json").await;
        let provider = provider_with(&server);
        let hits = provider.search("hxh", None).await.unwrap();
        assert_eq!(hits.len(), 2);
        assert!(hits.iter().any(|h| h.provider_id == HUNTER));
    }

    #[tokio::test]
    async fn search_filters_by_type() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/anime"))
            .and(query_param("q", "movie-title"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(fixture("jikan", "search_anime.json")),
            )
            .mount(&server)
            .await;
        let provider = provider_with(&server);
        // The fixture has one Movie (GITS) and one TV (HxH).
        let hits = provider
            .search("movie-title", Some(ContentType::Movie))
            .await
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits.iter().all(|h| h.content_type == ContentType::Movie));
    }

    #[tokio::test]
    async fn search_for_non_anime_domains_is_empty_without_requests() {
        let server = MockServer::start().await;
        let provider = provider_with(&server);
        let hits = provider
            .search("manga", Some(ContentType::Manga))
            .await
            .unwrap();
        assert!(hits.is_empty());
    }

    #[tokio::test]
    async fn get_details_normalizes() {
        let server = MockServer::start().await;
        mount(&server, &format!("/anime/{HUNTER}"), "details_anime.json").await;
        let provider = provider_with(&server);
        let media = provider.get_details(HUNTER).await.unwrap();
        assert_eq!(media.provider_id, HUNTER);
        assert_eq!(media.title_main, "Hunter x Hunter (2011)");
        assert_eq!(media.content_type, ContentType::Anime);
        assert_eq!(
            media.pub_status,
            crate::domain::enums::MediaStatus::Completed
        );
        assert_eq!(media.release_year, Some(2011));
        assert_eq!(media.duration_min, Some(24));
        assert_eq!(media.ep_count, Some(148));
        assert!(media.people.iter().any(|p| p.name == "Madhouse"));
        assert!(media
            .external_ids
            .iter()
            .any(|e| e.provider().as_str() == "mal"));
    }

    #[tokio::test]
    async fn get_nodes_builds_episode_list() {
        let server = MockServer::start().await;
        mount(&server, &format!("/anime/{HUNTER}"), "details_anime.json").await;
        mount(
            &server,
            &format!("/anime/{HUNTER}/episodes"),
            "episodes.json",
        )
        .await;
        let provider = provider_with(&server);
        let nodes = provider.get_nodes(HUNTER).await.unwrap();
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].kind, crate::domain::enums::NodeKind::Episode);
        assert_eq!(nodes[0].number.as_deref(), Some("1"));
        assert_eq!(nodes[1].title.as_deref(), Some("Test × Test"));
    }

    #[tokio::test]
    async fn get_details_maps_404_to_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/anime/999999"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let provider = provider_with(&server);
        let err = provider.get_details("999999").await.unwrap_err();
        assert!(matches!(err, ProviderError::NotFound { .. }));
    }

    #[tokio::test]
    async fn get_details_rejects_non_numeric_ids() {
        let server = MockServer::start().await;
        let provider = provider_with(&server);
        let err = provider.get_details("abc").await.unwrap_err();
        assert!(matches!(err, ProviderError::InvalidResponse { .. }));
    }

    #[tokio::test]
    async fn get_external_ids_returns_mal() {
        let server = MockServer::start().await;
        let provider = provider_with(&server);
        let ids = provider.get_external_ids(HUNTER).await.unwrap();
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0].provider().as_str(), "mal");
        assert_eq!(ids[0].value(), HUNTER);
    }

    #[tokio::test]
    async fn works_under_the_coordinator() {
        let server = MockServer::start().await;
        mount(&server, "/anime", "search_anime.json").await;
        let provider = provider_with(&server);
        let entry = (jikan_config(), Arc::new(provider) as Arc<dyn Provider>);
        let coordinator = ProviderCoordinator::new(vec![entry]).unwrap();
        let outcome = coordinator
            .search_all("hxh", None, &coordinator.token())
            .await;
        assert_eq!(outcome.hits.len(), 2);
        assert!(outcome.hits.iter().all(|h| h.provider == "jikan"));
        assert!(outcome.failures.is_empty());
    }
}
