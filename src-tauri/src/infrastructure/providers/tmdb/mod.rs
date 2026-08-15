//! TMDB adapter (MISSION-055, API_PROVIDERS §4).
//!
//! The primary movies+TV provider: REST at `api.themoviedb.org/3`, free API
//! key, ~40 req/10 s. This adapter is a pure normalizer — it maps TMDB JSON
//! into the unified domain types and typed `ProviderError`s; all policy (rate
//! limit, timeout, retry/backoff, cancel) is applied by the
//! `ProviderCoordinator`. TV shows get a Season→Episode node tree via one
//! `/tv/{id}/season/{n}` call per season; movies have no nodes.
//!
//! TMDB ids are per-kind (movie 603 ≠ tv 603), so every provider id here is
//! kind-prefixed: `movie-<id>` / `tv-<id>`. Cross-provider reconciliation of
//! plain numeric TMDB ids from other providers (e.g. AniList's `TMDB` link) is
//! left to the identity/dedup mission.

pub mod client;
mod normalize;
mod response;

pub use client::{TmdbClient, APP_USER_AGENT};

use async_trait::async_trait;

use crate::application::providers::config::ProviderConfig;
use crate::domain::enums::ContentType;
use crate::domain::provider::capabilities::{AuthKind, ProviderCapabilities};
use crate::domain::provider::error::ProviderError;
use crate::domain::provider::types::{ProviderCandidate, ProviderMedia, ProviderNode};
use crate::domain::provider::Provider;
use crate::domain::value_objects::ExternalId;

pub const PROVIDER_ID: &str = "tmdb";
pub const ENDPOINT: &str = "https://api.themoviedb.org/3";
/// ~40 req/10 s documented per key → schedule at 4 rps to stay under it.
pub const REQUESTS_PER_SEC: f64 = 4.0;

/// The config the coordinator registers TMDB with. It serves movies and TV.
pub fn tmdb_config() -> ProviderConfig {
    ProviderConfig::new(PROVIDER_ID)
        .with_requests_per_sec(REQUESTS_PER_SEC)
        .with_content_types(vec![ContentType::Movie, ContentType::Tv])
}

pub struct TmdbProvider {
    client: TmdbClient,
    caps: ProviderCapabilities,
}

impl TmdbProvider {
    pub fn new(client: TmdbClient) -> Self {
        Self {
            client,
            caps: ProviderCapabilities {
                search: true,
                details: true,
                nodes: true,
                related: false, // recommendations have no domain relation kind
                reviews: false,
                images: true,
                seasonal: false,
                auth: AuthKind::Key,
            },
        }
    }

    /// Split a kind-prefixed id like `movie-603` / `tv-1396`.
    fn split_id(provider_id: &str) -> Result<(bool, i64), ProviderError> {
        let id = provider_id
            .strip_prefix("movie-")
            .map(|id| (true, id))
            .or_else(|| provider_id.strip_prefix("tv-").map(|id| (false, id)));
        let Some((is_movie, id)) = id else {
            return Err(Self::invalid_id(provider_id));
        };
        id.parse()
            .map(|n| (is_movie, n))
            .map_err(|_| Self::invalid_id(provider_id))
    }

    fn invalid_id(provider_id: &str) -> ProviderError {
        ProviderError::InvalidResponse {
            provider: PROVIDER_ID.to_string(),
            message: format!("expected a `movie-<id>`/`tv-<id>` TMDB id, got {provider_id:?}"),
        }
    }

    fn details_path(is_movie: bool, id: i64) -> String {
        if is_movie {
            format!("/movie/{id}")
        } else {
            format!("/tv/{id}")
        }
    }

    async fn fetch_details(
        &self,
        provider_id: &str,
    ) -> Result<(bool, response::MediaDetails), ProviderError> {
        let (is_movie, id) = Self::split_id(provider_id)?;
        let details: response::MediaDetails = self
            .client
            .get(
                &Self::details_path(is_movie, id),
                &[
                    ("language", "en-US"),
                    ("append_to_response", "credits,external_ids"),
                ],
            )
            .await?;
        Ok((is_movie, details))
    }
}

#[async_trait]
impl Provider for TmdbProvider {
    fn id(&self) -> String {
        PROVIDER_ID.to_string()
    }

    fn name(&self) -> &str {
        "TMDB"
    }

    fn capabilities(&self) -> &ProviderCapabilities {
        &self.caps
    }

    async fn search(
        &self,
        query: &str,
        content_type: Option<ContentType>,
    ) -> Result<Vec<ProviderCandidate>, ProviderError> {
        let data: response::SearchResponse = self
            .client
            .get(
                "/search/multi",
                &[("query", query), ("include_adult", "false"), ("page", "1")],
            )
            .await?;
        Ok(data
            .results
            .iter()
            .filter_map(|row| {
                let hit_type = normalize::content_type(row.media_type.as_deref());
                if content_type.is_some_and(|wanted| wanted != hit_type) {
                    return None;
                }
                normalize::candidate(row)
            })
            .collect())
    }

    async fn get_details(&self, provider_id: &str) -> Result<ProviderMedia, ProviderError> {
        let (is_movie, details) = self.fetch_details(provider_id).await?;
        Ok(normalize::media(&details, is_movie))
    }

    async fn get_nodes(&self, provider_id: &str) -> Result<Vec<ProviderNode>, ProviderError> {
        let (is_movie, id) = Self::split_id(provider_id)?;
        if is_movie {
            return Ok(Vec::new());
        }
        let details: response::MediaDetails = self
            .client
            .get(&Self::details_path(false, id), &[("language", "en-US")])
            .await?;
        let Some(seasons) = details.seasons else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        for season in seasons {
            let Some(season_number) = season.season_number.filter(|n| *n >= 1) else {
                continue; // season 0 = specials, out of scope for the node tree
            };
            let season_details: response::SeasonDetails = self
                .client
                .get(
                    &format!("/tv/{id}/season/{season_number}"),
                    &[("language", "en-US")],
                )
                .await?;
            out.push(normalize::season_tree(&season_details, id));
        }
        Ok(out)
    }

    async fn get_external_ids(&self, provider_id: &str) -> Result<Vec<ExternalId>, ProviderError> {
        let (is_movie, id) = Self::split_id(provider_id)?;
        let path = if is_movie {
            format!("/movie/{id}/external_ids")
        } else {
            format!("/tv/{id}/external_ids")
        };
        let ext: response::ExternalIds = self.client.get(&path, &[]).await?;
        Ok(normalize::external_ids(Some(&ext)))
    }

    // get_related stays on the trait default (→ `Unsupported`); TMDB exposes no
    // sequel/prequel/adaptation edges, only recommendations.
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::application::providers::coordinator::ProviderCoordinator;
    use crate::infrastructure::providers::test_support::tmdb_fixture;

    fn provider_with(server: &MockServer) -> TmdbProvider {
        let client = TmdbClient::with_endpoint(reqwest::Client::new(), server.uri())
            .with_api_key("test-key");
        TmdbProvider::new(client)
    }

    async fn mount(server: &MockServer, route: &str, fixture: &str) {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(ResponseTemplate::new(200).set_body_string(tmdb_fixture(fixture)))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn search_returns_filtered_candidates() {
        let server = MockServer::start().await;
        mount(&server, "/search/multi", "search_multi.json").await;
        let provider = provider_with(&server);

        let hits = provider.search("matrix", None).await.unwrap();
        assert_eq!(hits.len(), 3);
        assert!(hits.iter().any(|h| h.provider_id == "movie-603"));

        let movies = provider
            .search("matrix", Some(ContentType::Movie))
            .await
            .unwrap();
        assert!(movies.iter().all(|h| h.content_type == ContentType::Movie));
        assert_eq!(movies.len(), 2);
        assert!(movies.iter().any(|h| h.provider_id == "movie-603"));
    }

    #[tokio::test]
    async fn get_details_movie_normalizes() {
        let server = MockServer::start().await;
        mount(&server, "/movie/603", "details_movie.json").await;
        let provider = provider_with(&server);
        let media = provider.get_details("movie-603").await.unwrap();
        assert_eq!(media.provider_id, "movie-603");
        assert_eq!(media.title_main, "The Matrix");
        assert_eq!(media.content_type, ContentType::Movie);
        assert_eq!(media.release_year, Some(1999));
        assert_eq!(media.duration_min, Some(136));
        assert!(!media.people.is_empty());
        assert!(!media.external_ids.is_empty());
    }

    #[tokio::test]
    async fn get_details_tv_normalizes() {
        let server = MockServer::start().await;
        mount(&server, "/tv/1396", "details_tv.json").await;
        let provider = provider_with(&server);
        let media = provider.get_details("tv-1396").await.unwrap();
        assert_eq!(media.provider_id, "tv-1396");
        assert_eq!(media.title_main, "Breaking Bad");
        assert_eq!(media.content_type, ContentType::Tv);
        assert_eq!(media.ep_count, Some(62));
        assert_eq!(
            media.pub_status,
            crate::domain::enums::MediaStatus::Completed
        );
        assert!(media
            .people
            .iter()
            .any(|p| p.role == crate::domain::enums::PersonRole::Network));
    }

    #[tokio::test]
    async fn get_nodes_tv_builds_season_episode_tree() {
        let server = MockServer::start().await;
        mount(&server, "/tv/1396", "details_tv.json").await;
        mount(&server, "/tv/1396/season/1", "season_1.json").await;
        let provider = provider_with(&server);
        let nodes = provider.get_nodes("tv-1396").await.unwrap();
        assert_eq!(nodes.len(), 1, "fixture carries one season");
        assert_eq!(nodes[0].kind, crate::domain::enums::NodeKind::Season);
        assert_eq!(nodes[0].id, "tv-1396-s1");
        assert_eq!(nodes[0].children.len(), 2);
        assert_eq!(nodes[0].children[0].id, "tv-1396-s1e1");
    }

    #[tokio::test]
    async fn get_nodes_movie_returns_empty_without_requests() {
        let server = MockServer::start().await;
        let provider = provider_with(&server);
        let nodes = provider.get_nodes("movie-603").await.unwrap();
        assert!(nodes.is_empty());
    }

    #[tokio::test]
    async fn get_external_ids_routes_by_kind() {
        let server = MockServer::start().await;
        mount(
            &server,
            "/movie/603/external_ids",
            "external_ids_movie.json",
        )
        .await;
        mount(&server, "/tv/1396/external_ids", "external_ids_tv.json").await;
        let provider = provider_with(&server);
        let movie = provider.get_external_ids("movie-603").await.unwrap();
        assert!(movie.iter().any(|e| e.provider().as_str() == "imdb"));
        let tv = provider.get_external_ids("tv-1396").await.unwrap();
        assert!(tv.iter().any(|e| e.provider().as_str() == "tvdb"));
    }

    #[tokio::test]
    async fn get_details_maps_404_to_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/movie/999"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let provider = provider_with(&server);
        let err = provider.get_details("movie-999").await.unwrap_err();
        assert!(matches!(err, ProviderError::NotFound { .. }));
    }

    #[tokio::test]
    async fn malformed_provider_id_is_invalid_response() {
        let server = MockServer::start().await;
        let provider = provider_with(&server);
        let err = provider.get_details("603").await.unwrap_err();
        assert!(matches!(err, ProviderError::InvalidResponse { .. }));
    }

    #[tokio::test]
    async fn works_under_the_coordinator() {
        let server = MockServer::start().await;
        mount(&server, "/search/multi", "search_multi.json").await;
        let provider = provider_with(&server);
        let entry = (tmdb_config(), Arc::new(provider) as Arc<dyn Provider>);
        let coordinator = ProviderCoordinator::new(vec![entry]).unwrap();
        let outcome = coordinator
            .search_all("matrix", Some(ContentType::Movie), &coordinator.token())
            .await;
        assert_eq!(outcome.hits.len(), 2);
        assert!(outcome.hits.iter().all(|h| h.provider == "tmdb"));
        assert!(outcome.failures.is_empty());
    }
}
