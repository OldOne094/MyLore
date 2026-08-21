//! Bangumi v0 transport (MISSION-066, API_PROVIDERS §13).
//!
//! Thin: one typed call per endpoint, then `from_http_status` error mapping.
//! All policy (rate limit, timeout, retry/backoff, cancel) lives in the
//! coordinator above this. Bangumi's anonymous reads need no auth (registered
//! as `AuthKind::None`), so unlike the keyed adapters there is no token on the
//! client — search is a JSON POST (`/v0/search/subjects`), the rest are GETs.

use serde::de::DeserializeOwned;
use serde_json::json;

use crate::domain::provider::error::ProviderError;

use super::response::{PagedEpisode, PagedSubject, RelatedSubject, Subject};
use super::PROVIDER_ID;

/// A polite, identifiable User-Agent.
pub const APP_USER_AGENT: &str = concat!(
    "MyLore/",
    env!("CARGO_PKG_VERSION"),
    " (local-first media tracker; Bangumi metadata)"
);

/// A reqwest-backed Bangumi client. `Clone` is cheap (one connection pool).
#[derive(Clone)]
pub struct BangumiClient {
    http: reqwest::Client,
    endpoint: String,
}

impl Default for BangumiClient {
    fn default() -> Self {
        Self::new()
    }
}

impl BangumiClient {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .user_agent(APP_USER_AGENT)
            .build()
            .expect("reqwest client builds");
        Self {
            http,
            endpoint: super::ENDPOINT.to_string(),
        }
    }

    /// Test hook: point the client at a local endpoint (wiremock).
    pub fn with_endpoint(http: reqwest::Client, endpoint: impl Into<String>) -> Self {
        Self {
            http,
            endpoint: endpoint.into(),
        }
    }

    /// Search subjects. `types` is the `filter.type` subject-type set (1 book,
    /// 2 anime); the experimental search API ignores unknown fields, so the
    /// body sticks to the documented `keyword`/`sort`/`filter` shape and
    /// pagination travels as `limit`/`offset` query params.
    pub(crate) async fn search_subjects(
        &self,
        keyword: &str,
        types: &[i64],
    ) -> Result<PagedSubject, ProviderError> {
        let body = json!({
            "keyword": keyword,
            "sort": "match",
            "filter": { "type": types, "nsfw": false }
        });
        let data = self
            .http
            .post(format!("{}/v0/search/subjects", self.endpoint))
            .query(&[("limit", "20"), ("offset", "0")])
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Transport {
                provider: PROVIDER_ID.to_string(),
                message: e.to_string(),
            })?;
        self.read_json(data).await
    }

    /// Full subject detail.
    pub(crate) async fn get_subject(&self, id: i64) -> Result<Subject, ProviderError> {
        let data = self
            .http
            .get(format!("{}/v0/subjects/{id}", self.endpoint))
            .send()
            .await
            .map_err(|e| ProviderError::Transport {
                provider: PROVIDER_ID.to_string(),
                message: e.to_string(),
            })?;
        self.read_json(data).await
    }

    /// The subject's episode/chapter feed (`GET /v0/episodes` — the route is
    /// *not* nested under `/v0/subjects/{id}/`; that path 404s).
    pub(crate) async fn get_episodes(&self, id: i64) -> Result<PagedEpisode, ProviderError> {
        let subject_id = id.to_string();
        let data = self
            .http
            .get(format!("{}/v0/episodes", self.endpoint))
            .query(&[
                ("subject_id", subject_id.as_str()),
                ("limit", "200"),
                ("offset", "0"),
            ])
            .send()
            .await
            .map_err(|e| ProviderError::Transport {
                provider: PROVIDER_ID.to_string(),
                message: e.to_string(),
            })?;
        self.read_json(data).await
    }

    /// Related-subject edges.
    pub(crate) async fn get_related(&self, id: i64) -> Result<Vec<RelatedSubject>, ProviderError> {
        let data = self
            .http
            .get(format!("{}/v0/subjects/{id}/subjects", self.endpoint))
            .send()
            .await
            .map_err(|e| ProviderError::Transport {
                provider: PROVIDER_ID.to_string(),
                message: e.to_string(),
            })?;
        self.read_json(data).await
    }

    /// Status→error mapping + JSON body parsing. Maps 401/403→AuthRequired,
    /// 404→NotFound, 429→RateLimited, 5xx→ProviderDown; malformed bodies →
    /// `InvalidResponse`.
    async fn read_json<T: DeserializeOwned>(
        &self,
        response: reqwest::Response,
    ) -> Result<T, ProviderError> {
        let status = response.status();
        if !status.is_success() {
            return Err(ProviderError::from_http_status(
                PROVIDER_ID,
                status.as_u16(),
            ));
        }
        let text = response
            .text()
            .await
            .map_err(|e| ProviderError::InvalidResponse {
                provider: PROVIDER_ID.to_string(),
                message: e.to_string(),
            })?;
        serde_json::from_str(&text).map_err(|e| ProviderError::InvalidResponse {
            provider: PROVIDER_ID.to_string(),
            message: format!("invalid JSON: {e}"),
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use wiremock::matchers::{body_partial_json, header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::infrastructure::providers::test_support::fixture;

    fn client_with(server: &MockServer) -> BangumiClient {
        let http = reqwest::Client::builder()
            .user_agent(APP_USER_AGENT)
            .build()
            .unwrap();
        BangumiClient::with_endpoint(http, server.uri())
    }

    #[tokio::test]
    async fn search_posts_filtered_body_with_ua() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v0/search/subjects"))
            .and(query_param("limit", "20"))
            .and(header("user-agent", APP_USER_AGENT))
            .and(body_partial_json(json!({ "keyword": "dune" })))
            .and(body_partial_json(json!({ "filter": { "type": [1, 2] } })))
            .and(body_partial_json(json!({ "sort": "match" })))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(fixture("bangumi", "search_subjects.json")),
            )
            .mount(&server)
            .await;

        let page = client_with(&server)
            .search_subjects("dune", &[1, 2])
            .await
            .unwrap();
        assert_eq!(page.data.len(), 2);
        assert_eq!(page.data[0].id, 1902);
    }

    #[tokio::test]
    async fn get_subject_parses_detail() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v0/subjects/211567"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(fixture("bangumi", "subject_detail.json")),
            )
            .mount(&server)
            .await;

        let subject = client_with(&server).get_subject(211567).await.unwrap();
        assert_eq!(subject.name, "3月のライオン 第2シリーズ");
        assert_eq!(subject.r#type, 2);
    }

    #[tokio::test]
    async fn get_episodes_hits_the_flat_route() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v0/episodes"))
            .and(query_param("subject_id", "211567"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(fixture("bangumi", "episodes.json")),
            )
            .mount(&server)
            .await;

        let page = client_with(&server).get_episodes(211567).await.unwrap();
        assert_eq!(
            page.data.len(),
            4,
            "raw feed rows; type filtering is normalize's job"
        );
    }

    #[tokio::test]
    async fn get_related_parses_edges() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v0/subjects/211567/subjects"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(fixture("bangumi", "relations.json")),
            )
            .mount(&server)
            .await;

        let rels = client_with(&server).get_related(211567).await.unwrap();
        assert_eq!(rels.len(), 3);
        assert_eq!(rels[0].id, 1902);
    }

    #[tokio::test]
    async fn maps_429_to_rate_limited() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v0/subjects/1"))
            .respond_with(ResponseTemplate::new(429))
            .mount(&server)
            .await;

        let err = client_with(&server).get_subject(1).await.unwrap_err();
        assert!(matches!(err, ProviderError::RateLimited { .. }));
    }

    #[tokio::test]
    async fn maps_404_to_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v0/subjects/99999"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let err = client_with(&server).get_subject(99999).await.unwrap_err();
        assert!(matches!(err, ProviderError::NotFound { .. }));
    }

    #[tokio::test]
    async fn maps_503_to_provider_down() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v0/subjects/1"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;

        let err = client_with(&server).get_subject(1).await.unwrap_err();
        assert!(matches!(err, ProviderError::ProviderDown { .. }));
    }

    #[tokio::test]
    async fn maps_invalid_json_to_invalid_response() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v0/subjects/1"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
            .mount(&server)
            .await;

        let err = client_with(&server).get_subject(1).await.unwrap_err();
        assert!(matches!(err, ProviderError::InvalidResponse { .. }));
    }
}
