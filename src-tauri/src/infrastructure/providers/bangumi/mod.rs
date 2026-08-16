//! Bangumi adapter (MISSION-066, API_PROVIDERS §13).
//!
//! The optional CN ACGN provider (anime/manga/light novels/web novels/games).
//! Anonymous reads need no key, so the adapter is registered as
//! `AuthKind::None` and appears in the settings UI without a key field.
//! Provider ids are numeric subject ids (`https://bgm.tv/subject/{id}`).
//!
//! Like all adapters this is a pure normalizer; all policy (rate limit,
//! timeout, retry/backoff, cancel) is applied by the `ProviderCoordinator`.
//! Bangumi's documented limits are ~1 rps (15/60 s) — the config throttles to
//! that and the coordinator caches details. The chapter tree and adaptation
//! edges are the two extras over a pure metadata provider: books/chapters and
//! anime/episodes both come from `GET /v0/episodes`, and `GET
//! /v0/subjects/{id}/subjects` exposes `前传`/`续集`/`原作`… relation edges.

pub mod client;
mod normalize;
mod response;

pub use client::{BangumiClient, APP_USER_AGENT};

use async_trait::async_trait;

use crate::application::providers::config::ProviderConfig;
use crate::domain::enums::ContentType;
use crate::domain::provider::capabilities::{AuthKind, ProviderCapabilities};
use crate::domain::provider::error::ProviderError;
use crate::domain::provider::types::{
    ProviderCandidate, ProviderMedia, ProviderNode, ProviderRelation,
};
use crate::domain::provider::Provider;

pub const PROVIDER_ID: &str = "bangumi";
pub const ENDPOINT: &str = "https://api.bgm.tv";
/// Bangumi documents 1 req/s (15 per 60 s, 80 per 10 min) → throttle to 1 rps.
pub const REQUESTS_PER_SEC: f64 = 1.0;

/// Canonical human-facing subject page.
pub(crate) fn subject_url(id: i64) -> String {
    format!("https://bgm.tv/subject/{id}")
}

/// The config the coordinator registers Bangumi with: CN ACGN fallback for
/// anime, manga and the novel domains.
pub fn bangumi_config() -> ProviderConfig {
    ProviderConfig::new(PROVIDER_ID)
        .with_requests_per_sec(REQUESTS_PER_SEC)
        .with_content_types(vec![
            ContentType::Anime,
            ContentType::Manga,
            ContentType::Novel,
            ContentType::WebNovel,
            ContentType::Book,
        ])
}

pub struct BangumiProvider {
    client: BangumiClient,
    caps: ProviderCapabilities,
}

impl BangumiProvider {
    pub fn new(client: BangumiClient) -> Self {
        Self {
            client,
            caps: ProviderCapabilities {
                search: true,
                details: true,
                nodes: true,   // chapters (books) / episodes (anime)
                related: true, // 前传/续集/原作… adaptation edges
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
            message: format!("expected a numeric Bangumi subject id, got {provider_id:?}"),
        }
    }

    /// Validate + normalize a subject id (numeric; `a/b`, `-1`, blanks rejected).
    fn parse_subject_id(&self, provider_id: &str) -> Result<i64, ProviderError> {
        provider_id
            .parse::<i64>()
            .map_err(|_| Self::invalid_id(provider_id))
    }

    async fn fetch_subject(&self, provider_id: &str) -> Result<response::Subject, ProviderError> {
        let id = self.parse_subject_id(provider_id)?;
        self.client.get_subject(id).await
    }
}

#[async_trait]
impl Provider for BangumiProvider {
    fn id(&self) -> String {
        PROVIDER_ID.to_string()
    }

    fn name(&self) -> &str {
        "Bangumi"
    }

    fn capabilities(&self) -> &ProviderCapabilities {
        &self.caps
    }

    async fn search(
        &self,
        query: &str,
        content_type: Option<ContentType>,
    ) -> Result<Vec<ProviderCandidate>, ProviderError> {
        let types: Vec<i64> = match content_type {
            Some(ContentType::Anime) => vec![2],
            Some(
                ContentType::Book | ContentType::Novel | ContentType::WebNovel | ContentType::Manga,
            ) => vec![1],
            Some(_) => return Ok(Vec::new()),
            None => vec![1, 2],
        };
        let page = self.client.search_subjects(query, &types).await?;
        Ok(page.data.iter().filter_map(normalize::candidate).collect())
    }

    async fn get_details(&self, provider_id: &str) -> Result<ProviderMedia, ProviderError> {
        let subject = self.fetch_subject(provider_id).await?;
        normalize::media(&subject, provider_id).ok_or_else(|| Self::invalid_id(provider_id))
    }

    async fn get_nodes(&self, provider_id: &str) -> Result<Vec<ProviderNode>, ProviderError> {
        // The node kind follows the subject's content type (episode vs chapter),
        // which only the detail payload carries → subject + episodes.
        let subject = self.fetch_subject(provider_id).await?;
        let ct = normalize::content_type(subject.r#type, subject.platform.as_deref());
        let id = self.parse_subject_id(provider_id)?;
        let episodes = self.client.get_episodes(id).await?;
        Ok(normalize::nodes(&episodes, provider_id, ct))
    }

    async fn get_related(&self, provider_id: &str) -> Result<Vec<ProviderRelation>, ProviderError> {
        let id = self.parse_subject_id(provider_id)?;
        let related = self.client.get_related(id).await?;
        Ok(normalize::relations(&related))
    }

    // get_external_ids stays on the trait default (→ `Unsupported`): the v0
    // subject payload carries no cross-provider ids (AniList links *to* Bangumi,
    // not the reverse).
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::application::providers::coordinator::ProviderCoordinator;
    use crate::domain::enums::{MediaRelationKind, NodeKind};
    use crate::infrastructure::providers::test_support::bangumi_fixture;

    const SANGATSU: &str = "211567";

    fn provider_with(server: &MockServer) -> BangumiProvider {
        BangumiProvider::new(BangumiClient::with_endpoint(
            reqwest::Client::new(),
            server.uri(),
        ))
    }

    async fn mount(server: &MockServer, route: &str, fixture: &str) {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(ResponseTemplate::new(200).set_body_string(bangumi_fixture(fixture)))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn search_returns_candidates() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v0/search/subjects"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(bangumi_fixture("search_subjects.json")),
            )
            .mount(&server)
            .await;
        let provider = provider_with(&server);
        let hits = provider.search("sangatsu", None).await.unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].provider_id, "1902");
        assert_eq!(hits[0].content_type, ContentType::Manga);
        assert_eq!(hits[1].content_type, ContentType::Anime);
    }

    #[tokio::test]
    async fn search_serves_acgn_domains_and_short_circuits_others() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v0/search/subjects"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(bangumi_fixture("search_subjects.json")),
            )
            .mount(&server)
            .await;
        let provider = provider_with(&server);
        for ct in [
            ContentType::Anime,
            ContentType::Manga,
            ContentType::Novel,
            ContentType::WebNovel,
            ContentType::Book,
        ] {
            let hits = provider.search("sangatsu", Some(ct)).await.unwrap();
            assert!(!hits.is_empty(), "{ct:?} should route here");
        }
        let hits = provider
            .search("cowboy", Some(ContentType::Movie))
            .await
            .unwrap();
        assert!(hits.is_empty());
    }

    #[tokio::test]
    async fn get_details_normalizes() {
        let server = MockServer::start().await;
        mount(
            &server,
            &format!("/v0/subjects/{SANGATSU}"),
            "subject_detail.json",
        )
        .await;
        let provider = provider_with(&server);
        let media = provider.get_details(SANGATSU).await.unwrap();
        assert_eq!(media.provider_id, SANGATSU);
        assert_eq!(media.title_main, "3月的狮子 第二季");
        assert_eq!(media.content_type, ContentType::Anime);
        assert_eq!(media.ep_count, Some(22));
        assert!(media.people.iter().any(|p| p.name == "新房昭之"));
        assert_eq!(media.url.as_deref(), Some("https://bgm.tv/subject/211567"));
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
    async fn get_nodes_builds_anime_episode_tree() {
        let server = MockServer::start().await;
        mount(
            &server,
            &format!("/v0/subjects/{SANGATSU}"),
            "subject_detail.json",
        )
        .await;
        Mock::given(method("GET"))
            .and(path("/v0/episodes"))
            .and(query_param("subject_id", SANGATSU))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(bangumi_fixture("episodes.json")),
            )
            .mount(&server)
            .await;
        let provider = provider_with(&server);
        let nodes = provider.get_nodes(SANGATSU).await.unwrap();
        assert_eq!(nodes.len(), 3, "2 main + 1 special; OP/ED dropped");
        assert_eq!(nodes[0].kind, NodeKind::Episode);
        assert_eq!(nodes[0].number.as_deref(), Some("1"));
        assert!(!nodes[0].is_special);
        assert!(nodes[2].is_special);
    }

    #[tokio::test]
    async fn get_related_maps_adaptation_edges() {
        let server = MockServer::start().await;
        mount(
            &server,
            &format!("/v0/subjects/{SANGATSU}/subjects"),
            "relations.json",
        )
        .await;
        let provider = provider_with(&server);
        let rels = provider.get_related(SANGATSU).await.unwrap();
        assert_eq!(rels.len(), 3);
        assert_eq!(rels[0].relation, MediaRelationKind::Adaptation);
        assert_eq!(rels[1].relation, MediaRelationKind::Prequel);
        assert_eq!(rels[2].relation, MediaRelationKind::Other);
    }

    #[tokio::test]
    async fn works_under_the_coordinator() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v0/search/subjects"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(bangumi_fixture("search_subjects.json")),
            )
            .mount(&server)
            .await;
        let provider = provider_with(&server);
        let entry = (bangumi_config(), Arc::new(provider) as Arc<dyn Provider>);
        let coordinator = ProviderCoordinator::new(vec![entry]).unwrap();
        let outcome = coordinator
            .search_all("sangatsu", Some(ContentType::Anime), &coordinator.token())
            .await;
        assert_eq!(outcome.hits.len(), 2);
        assert!(outcome.hits.iter().all(|h| h.provider == "bangumi"));
        assert!(outcome.failures.is_empty());
    }
}
