//! MangaDex adapter (MISSION-056, API_PROVIDERS §3).
//!
//! The manga/manhwa/manhua/novel provider: public keyless REST at
//! `api.mangadex.org`, ~5 req/s. This adapter is a pure normalizer — it maps
//! MangaDex JSON into the unified domain types and typed `ProviderError`s; all
//! policy (rate limit, timeout, retry/backoff, cancel) is applied by the
//! `ProviderCoordinator`. Chapter/volume node trees come from the feed
//! (`/manga/{id}/feed`); MangaDex exposes no sequel/prequel edges, so `related`
//! stays off. MangaDex ids are UUIDs, unique across kinds — no prefix needed.

pub mod client;
mod normalize;
mod response;

pub use client::{MangaDexClient, APP_USER_AGENT};

use async_trait::async_trait;

use crate::application::providers::config::ProviderConfig;
use crate::domain::enums::ContentType;
use crate::domain::provider::capabilities::{AuthKind, ProviderCapabilities};
use crate::domain::provider::error::ProviderError;
use crate::domain::provider::types::{ProviderCandidate, ProviderMedia, ProviderNode};
use crate::domain::provider::Provider;
use crate::domain::value_objects::ExternalId;

pub const PROVIDER_ID: &str = "mangadex";
pub const ENDPOINT: &str = "https://api.mangadex.org";
/// ~5 req/s documented public limit → schedule at 4 rps to stay under it.
pub const REQUESTS_PER_SEC: f64 = 4.0;

/// The config the coordinator registers MangaDex with. It serves the comic
/// domains incl. manhwa/manhua and light/web novels.
pub fn mangadex_config() -> ProviderConfig {
    ProviderConfig::new(PROVIDER_ID)
        .with_requests_per_sec(REQUESTS_PER_SEC)
        .with_content_types(vec![
            ContentType::Manga,
            ContentType::Manhwa,
            ContentType::Manhua,
            ContentType::Novel,
        ])
}

pub struct MangaDexProvider {
    client: MangaDexClient,
    caps: ProviderCapabilities,
}

impl MangaDexProvider {
    pub fn new(client: MangaDexClient) -> Self {
        Self {
            client,
            caps: ProviderCapabilities {
                search: true,
                details: true,
                nodes: true,
                related: false, // no sequel/prequel/adaptation edges on MangaDex
                reviews: false,
                images: true,
                seasonal: false,
                auth: AuthKind::None,
            },
        }
    }

    /// The `format[]` filter for a domain content type. Domains MangaDex
    /// doesn't serve map to `None` (callers short-circuit).
    fn format_filter(content_type: Option<ContentType>) -> Option<&'static [&'static str]> {
        match content_type {
            Some(ContentType::Manga) => Some(&["manga", "one_shot", "doujinshi"]),
            Some(ContentType::Manhwa) => Some(&["manhwa"]),
            Some(ContentType::Manhua) => Some(&["manhua"]),
            Some(ContentType::Novel) => Some(&["novel"]),
            _ => None,
        }
    }

    async fn fetch_manga(&self, provider_id: &str) -> Result<response::Manga, ProviderError> {
        let data: response::MangaSingleResponse = self
            .client
            .get(
                &format!("/manga/{provider_id}"),
                &[
                    ("includes[]", "author"),
                    ("includes[]", "artist"),
                    ("includes[]", "cover_art"),
                ],
            )
            .await?;
        data.data.ok_or_else(|| ProviderError::NotFound {
            provider: PROVIDER_ID.to_string(),
        })
    }
}

#[async_trait]
impl Provider for MangaDexProvider {
    fn id(&self) -> String {
        PROVIDER_ID.to_string()
    }

    fn name(&self) -> &str {
        "MangaDex"
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
                ContentType::Manga | ContentType::Manhwa | ContentType::Manhua | ContentType::Novel
            )
        }) {
            return Ok(Vec::new());
        }
        let mut params: Vec<(&str, &str)> = vec![
            ("title", query),
            ("limit", "20"),
            ("includes[]", "cover_art"),
            ("availableTranslatedLanguage[]", "en"),
            ("contentRating[]", "safe"),
            ("contentRating[]", "suggestive"),
            ("contentRating[]", "erotica"),
            ("contentRating[]", "pornographic"),
            ("order[relevance]", "desc"),
        ];
        if let Some(formats) = Self::format_filter(content_type) {
            for format in formats {
                params.push(("format[]", format));
            }
        }
        let data: response::MangaListResponse = self.client.get("/manga", &params).await?;
        Ok(data
            .data
            .iter()
            .filter_map(|manga| {
                let hit_type = normalize::content_type(manga.attributes.format.as_deref());
                if content_type.is_some_and(|wanted| wanted != hit_type) {
                    return None;
                }
                normalize::candidate(manga)
            })
            .collect())
    }

    async fn get_details(&self, provider_id: &str) -> Result<ProviderMedia, ProviderError> {
        Ok(normalize::media(&self.fetch_manga(provider_id).await?))
    }

    async fn get_nodes(&self, provider_id: &str) -> Result<Vec<ProviderNode>, ProviderError> {
        let data: response::FeedResponse = self
            .client
            .get(
                &format!("/manga/{provider_id}/feed"),
                &[
                    ("translatedLanguage[]", "en"),
                    ("limit", "500"),
                    ("order[volume]", "asc"),
                    ("order[chapter]", "asc"),
                ],
            )
            .await?;
        Ok(normalize::nodes(&data.data, provider_id))
    }

    async fn get_external_ids(&self, provider_id: &str) -> Result<Vec<ExternalId>, ProviderError> {
        let manga = self.fetch_manga(provider_id).await?;
        Ok(normalize::external_ids(manga.attributes.links.as_ref()))
    }

    // get_related stays on the trait default (→ `Unsupported`).
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::application::providers::coordinator::ProviderCoordinator;
    use crate::infrastructure::providers::test_support::mangadex_fixture;

    const BERSERK: &str = "11111111-1111-1111-1111-111111111111";

    fn provider_with(server: &MockServer) -> MangaDexProvider {
        MangaDexProvider::new(MangaDexClient::with_endpoint(
            reqwest::Client::new(),
            server.uri(),
        ))
    }

    async fn mount(server: &MockServer, route: &str, fixture: &str) {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(ResponseTemplate::new(200).set_body_string(mangadex_fixture(fixture)))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn search_returns_candidates() {
        let server = MockServer::start().await;
        mount(&server, "/manga", "search_manga.json").await;
        let provider = provider_with(&server);
        let hits = provider.search("berserk", None).await.unwrap();
        assert_eq!(hits.len(), 3);
        assert!(hits.iter().any(|h| h.provider_id == BERSERK));
    }

    #[tokio::test]
    async fn search_filters_by_format_param() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/manga"))
            .and(query_param("format[]", "manhwa"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(mangadex_fixture("search_manga.json")),
            )
            .mount(&server)
            .await;
        let provider = provider_with(&server);
        let hits = provider
            .search("leveling", Some(ContentType::Manhwa))
            .await
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits.iter().all(|h| h.content_type == ContentType::Manhwa));
    }

    #[tokio::test]
    async fn search_for_unsupported_domains_is_empty_without_requests() {
        let server = MockServer::start().await;
        let provider = provider_with(&server);
        let hits = provider
            .search("cowboy", Some(ContentType::Anime))
            .await
            .unwrap();
        assert!(hits.is_empty());
        let hits = provider
            .search("cowboy", Some(ContentType::Movie))
            .await
            .unwrap();
        assert!(hits.is_empty());
    }

    #[tokio::test]
    async fn get_details_normalizes() {
        let server = MockServer::start().await;
        mount(&server, &format!("/manga/{BERSERK}"), "details_manga.json").await;
        let provider = provider_with(&server);
        let media = provider.get_details(BERSERK).await.unwrap();
        assert_eq!(media.provider_id, BERSERK);
        assert_eq!(media.title_main, "Berserk");
        assert_eq!(media.content_type, ContentType::Manga);
        assert_eq!(media.release_year, Some(1989));
        assert!(!media.people.is_empty());
        assert!(!media.external_ids.is_empty());
    }

    #[tokio::test]
    async fn get_nodes_builds_volume_chapter_tree() {
        let server = MockServer::start().await;
        mount(
            &server,
            &format!("/manga/{BERSERK}/feed"),
            "chapter_feed.json",
        )
        .await;
        let provider = provider_with(&server);
        let nodes = provider.get_nodes(BERSERK).await.unwrap();
        assert_eq!(nodes.len(), 3);
        assert_eq!(nodes[0].kind, crate::domain::enums::NodeKind::Volume);
        assert_eq!(nodes[0].children.len(), 2);
        assert_eq!(nodes[2].kind, crate::domain::enums::NodeKind::Chapter);
    }

    #[tokio::test]
    async fn get_external_ids_from_links() {
        let server = MockServer::start().await;
        mount(&server, &format!("/manga/{BERSERK}"), "details_manga.json").await;
        let provider = provider_with(&server);
        let ids = provider.get_external_ids(BERSERK).await.unwrap();
        assert!(ids.iter().any(|e| e.provider().as_str() == "mal"));
        assert!(ids.iter().any(|e| e.provider().as_str() == "anilist"));
    }

    #[tokio::test]
    async fn get_details_maps_404_to_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/manga/not-here"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let provider = provider_with(&server);
        let err = provider.get_details("not-here").await.unwrap_err();
        assert!(matches!(err, ProviderError::NotFound { .. }));
    }

    #[tokio::test]
    async fn works_under_the_coordinator() {
        let server = MockServer::start().await;
        mount(&server, "/manga", "search_manga.json").await;
        let provider = provider_with(&server);
        let entry = (mangadex_config(), Arc::new(provider) as Arc<dyn Provider>);
        let coordinator = ProviderCoordinator::new(vec![entry]).unwrap();
        let outcome = coordinator
            .search_all("berserk", None, &coordinator.token())
            .await;
        assert_eq!(outcome.hits.len(), 3);
        assert!(outcome.hits.iter().all(|h| h.provider == "mangadex"));
        assert!(outcome.failures.is_empty());
    }
}
