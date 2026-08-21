//! NovelUpdates adapter (MISSION-065, API_PROVIDERS §14).
//!
//! The web/light-novel provider. NovelUpdates is server-rendered HTML (no JSON
//! metadata API), so this adapter parses the same pages the maintained LNReader
//! `novelupdates` plugin reads, at a conservative ~1 req/s. It is a pure
//! normalizer — it maps parsed HTML into the unified domain types and typed
//! `ProviderError`s; all policy (rate limit, timeout, retry/backoff, cancel) is
//! applied by the `ProviderCoordinator`. Provider ids are series slugs
//! (`/series/{slug}/` → `{slug}`). Chapter trees come from the `admin-ajax.php`
//! POST keyed by the numeric post id extracted from the series page; NU exposes
//! no sequel/prequel edges, so `related` stays off.

pub mod client;
mod normalize;
mod response;

pub use client::{NovelUpdatesClient, APP_USER_AGENT};

use async_trait::async_trait;

use crate::application::providers::config::ProviderConfig;
use crate::domain::enums::ContentType;
use crate::domain::provider::capabilities::{AuthKind, ProviderCapabilities};
use crate::domain::provider::error::ProviderError;
use crate::domain::provider::types::{ProviderCandidate, ProviderMedia, ProviderNode};
use crate::domain::provider::Provider;

pub const PROVIDER_ID: &str = "novelupdates";
pub const ENDPOINT: &str = "https://www.novelupdates.com";
/// Conservative self-imposed rate for HTML scraping (API_PROVIDERS §14).
pub const REQUESTS_PER_SEC: f64 = 1.0;

/// Canonical human-facing page for a series slug.
pub(crate) fn page_url(slug: &str) -> String {
    format!("{ENDPOINT}/series/{slug}/")
}

/// The config the coordinator registers NovelUpdates with. It serves the
/// novel domains (printed/light + web novels).
pub fn novelupdates_config() -> ProviderConfig {
    ProviderConfig::new(PROVIDER_ID)
        .with_requests_per_sec(REQUESTS_PER_SEC)
        .with_content_types(vec![ContentType::Novel, ContentType::WebNovel])
}

pub struct NovelUpdatesProvider {
    client: NovelUpdatesClient,
    caps: ProviderCapabilities,
}

impl NovelUpdatesProvider {
    pub fn new(client: NovelUpdatesClient) -> Self {
        Self {
            client,
            caps: ProviderCapabilities {
                search: true,
                details: true,
                nodes: true,
                related: false, // no sequel/prequel/adaptation edges on NU
                reviews: false,
                images: true,
                seasonal: false,
                auth: AuthKind::None,
            },
        }
    }
}

#[async_trait]
impl Provider for NovelUpdatesProvider {
    fn id(&self) -> String {
        PROVIDER_ID.to_string()
    }

    fn name(&self) -> &str {
        "NovelUpdates"
    }

    fn capabilities(&self) -> &ProviderCapabilities {
        &self.caps
    }

    async fn search(
        &self,
        query: &str,
        content_type: Option<ContentType>,
    ) -> Result<Vec<ProviderCandidate>, ProviderError> {
        if content_type.is_some_and(|ct| !matches!(ct, ContentType::Novel | ContentType::WebNovel))
        {
            return Ok(Vec::new());
        }
        let html = self
            .client
            .get(
                "/series-finder/",
                &[
                    ("sf", "1"),
                    ("sh", query),
                    ("sort", "srank"),
                    ("order", "asc"),
                    ("pg", "1"),
                ],
            )
            .await?;
        Ok(response::parse_search_rows(&html)
            .iter()
            .filter_map(normalize::candidate)
            .collect())
    }

    async fn get_details(&self, provider_id: &str) -> Result<ProviderMedia, ProviderError> {
        let html = self
            .client
            .get(&format!("/series/{provider_id}/"), &[])
            .await?;
        let page = response::parse_series_page(&html).ok_or_else(|| ProviderError::NotFound {
            provider: PROVIDER_ID.to_string(),
        })?;
        Ok(normalize::media(&page, provider_id))
    }

    async fn get_nodes(&self, provider_id: &str) -> Result<Vec<ProviderNode>, ProviderError> {
        // The chapter feed needs the numeric post id, which only the series
        // page carries → one GET to resolve it, then the POST.
        let html = self
            .client
            .get(&format!("/series/{provider_id}/"), &[])
            .await?;
        let page = response::parse_series_page(&html).ok_or_else(|| ProviderError::NotFound {
            provider: PROVIDER_ID.to_string(),
        })?;
        if page.post_id.is_empty() {
            return Err(ProviderError::InvalidResponse {
                provider: PROVIDER_ID.to_string(),
                message: "series page had no `input#mypostid`".to_string(),
            });
        }
        let chapters = self
            .client
            .post_form(
                "/wp-admin/admin-ajax.php",
                &[
                    ("action", "nd_getchapters"),
                    ("mygrr", "0"),
                    ("mypostid", &page.post_id),
                ],
            )
            .await?;
        let labels = response::parse_chapter_labels(&chapters);
        Ok(normalize::nodes(&labels, provider_id))
    }

    // get_related, get_external_ids stay on the trait defaults (→ `Unsupported`).
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::application::providers::coordinator::ProviderCoordinator;
    use crate::infrastructure::providers::test_support::novelupdates_fixture;

    const DUNGEON_DEFENDER: &str = "dungeon-defender";

    fn provider_with(server: &MockServer) -> NovelUpdatesProvider {
        NovelUpdatesProvider::new(NovelUpdatesClient::with_endpoint(
            reqwest::Client::new(),
            server.uri(),
        ))
    }

    async fn mount(server: &MockServer, route: &str, fixture: &str) {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(ResponseTemplate::new(200).set_body_string(novelupdates_fixture(fixture)))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn search_returns_candidates() {
        let server = MockServer::start().await;
        mount(&server, "/series-finder/", "search_series.html").await;
        let provider = provider_with(&server);
        let hits = provider.search("dungeon", None).await.unwrap();
        assert_eq!(hits.len(), 2);
        let hit = hits
            .iter()
            .find(|h| h.provider_id == DUNGEON_DEFENDER)
            .unwrap();
        assert_eq!(hit.title, "Dungeon Defender");
        assert_eq!(hit.content_type, ContentType::Novel);
        assert!(hit.cover_url.is_some());
    }

    #[tokio::test]
    async fn search_sends_query_params() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/series-finder/"))
            .and(query_param("sh", "dungeon"))
            .and(query_param("sf", "1"))
            .and(query_param("pg", "1"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(novelupdates_fixture("search_series.html")),
            )
            .mount(&server)
            .await;
        let provider = provider_with(&server);
        let hits = provider.search("dungeon", None).await.unwrap();
        assert_eq!(hits.len(), 2);
    }

    #[tokio::test]
    async fn search_for_non_novel_domains_is_empty_without_requests() {
        let server = MockServer::start().await;
        let provider = provider_with(&server);
        for ct in [ContentType::Anime, ContentType::Movie, ContentType::Book] {
            let hits = provider.search("cowboy", Some(ct)).await.unwrap();
            assert!(hits.is_empty(), "{ct:?} should short-circuit");
        }
    }

    #[tokio::test]
    async fn get_details_normalizes() {
        let server = MockServer::start().await;
        mount(
            &server,
            &format!("/series/{DUNGEON_DEFENDER}/"),
            "series_dungeon_defender.html",
        )
        .await;
        let provider = provider_with(&server);
        let media = provider.get_details(DUNGEON_DEFENDER).await.unwrap();
        assert_eq!(media.provider_id, DUNGEON_DEFENDER);
        assert_eq!(media.title_main, "Dungeon Defender");
        assert_eq!(media.content_type, ContentType::WebNovel);
        assert_eq!(media.pub_status, crate::domain::enums::MediaStatus::Ongoing);
        assert!(!media.people.is_empty());
        assert!(!media.genres.is_empty());
    }

    #[tokio::test]
    async fn get_details_maps_missing_page_to_not_found() {
        let server = MockServer::start().await;
        mount(
            &server,
            &format!("/series/{DUNGEON_DEFENDER}/"),
            "captcha.html",
        )
        .await;
        let provider = provider_with(&server);
        // captcha is caught by the client before parsing
        let err = provider.get_details(DUNGEON_DEFENDER).await.unwrap_err();
        assert!(matches!(err, ProviderError::InvalidResponse { .. }));

        // a 200 page without the series markers → NotFound
        Mock::given(method("GET"))
            .and(path("/series/not-a-series/"))
            .respond_with(ResponseTemplate::new(200).set_body_string("<html>gone</html>"))
            .mount(&server)
            .await;
        let err = provider.get_details("not-a-series").await.unwrap_err();
        assert!(matches!(err, ProviderError::NotFound { .. }));
    }

    #[tokio::test]
    async fn get_nodes_resolves_post_id_and_builds_tree() {
        let server = MockServer::start().await;
        mount(
            &server,
            &format!("/series/{DUNGEON_DEFENDER}/"),
            "series_dungeon_defender.html",
        )
        .await;
        Mock::given(method("POST"))
            .and(path("/wp-admin/admin-ajax.php"))
            .and(wiremock::matchers::body_string(
                "action=nd_getchapters&mygrr=0&mypostid=42817",
            ))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(novelupdates_fixture("chapters_dungeon_defender.html")),
            )
            .mount(&server)
            .await;
        let provider = provider_with(&server);
        let nodes = provider.get_nodes(DUNGEON_DEFENDER).await.unwrap();
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].kind, crate::domain::enums::NodeKind::Volume);
        assert_eq!(nodes[0].children.len(), 3);
        assert!(nodes[1].is_special);
    }

    #[tokio::test]
    async fn works_under_the_coordinator() {
        let server = MockServer::start().await;
        mount(&server, "/series-finder/", "search_series.html").await;
        let provider = provider_with(&server);
        let entry = (
            novelupdates_config(),
            Arc::new(provider) as Arc<dyn Provider>,
        );
        let coordinator = ProviderCoordinator::new(vec![entry]).unwrap();
        let outcome = coordinator
            .search_all("dungeon", Some(ContentType::WebNovel), &coordinator.token())
            .await;
        assert_eq!(outcome.hits.len(), 2);
        assert!(outcome.hits.iter().all(|h| h.provider == "novelupdates"));
        assert!(outcome.failures.is_empty());
    }
}
