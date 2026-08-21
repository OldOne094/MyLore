//! MangaDex REST transport (MISSION-056).
//!
//! One GET per call, then typed error mapping. Policy (rate limit, timeout,
//! retry/backoff, cancel) lives in the coordinator above this; the client only
//! maps transport/HTTP/JSON outcomes to `ProviderError`. MangaDex public GETs
//! are keyless (API_PROVIDERS §3).

use serde::de::DeserializeOwned;

use crate::domain::provider::error::ProviderError;

use super::PROVIDER_ID;

/// A polite, identifiable User-Agent (MangaDex acceptable-use policy requires
/// crediting the API; a UA with contact info is expected).
pub const APP_USER_AGENT: &str = concat!(
    "MyLore/",
    env!("CARGO_PKG_VERSION"),
    " (local-first media tracker; MangaDex API)"
);

/// A reqwest-backed MangaDex client. `Clone` is cheap (one connection pool).
#[derive(Clone)]
pub struct MangaDexClient {
    http: reqwest::Client,
    endpoint: String,
}

impl Default for MangaDexClient {
    fn default() -> Self {
        Self::new()
    }
}

impl MangaDexClient {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .user_agent(APP_USER_AGENT)
            .build()
            .expect("reqwest client builds");
        Self::with_client(http)
    }

    pub fn with_client(http: reqwest::Client) -> Self {
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

    /// GET `endpoint + path` with query params (repeated keys like
    /// `contentRating[]` are preserved). Maps transport failures → `Transport`,
    /// HTTP status → `from_http_status` (404→NotFound, 429→RateLimited,
    /// 5xx→ProviderDown) and malformed JSON → `InvalidResponse`.
    pub async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, &str)],
    ) -> Result<T, ProviderError> {
        let response = self
            .http
            .get(format!("{}{}", self.endpoint, path))
            .query(params)
            .send()
            .await
            .map_err(|e| ProviderError::Transport {
                provider: PROVIDER_ID.to_string(),
                message: e.to_string(),
            })?;

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
            message: format!("invalid MangaDex payload: {e}"),
        })
    }
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::infrastructure::providers::test_support::fixture;

    fn client_with(server: &MockServer) -> MangaDexClient {
        MangaDexClient::with_endpoint(reqwest::Client::new(), server.uri())
    }

    #[tokio::test]
    async fn get_preserves_repeated_query_params_and_parses() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/manga"))
            .and(query_param("title", "berserk"))
            .and(query_param("contentRating[]", "safe"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(fixture("mangadex", "search_manga.json")),
            )
            .mount(&server)
            .await;
        let list: super::super::response::MangaListResponse = client_with(&server)
            .get(
                "/manga",
                &[("title", "berserk"), ("contentRating[]", "safe")],
            )
            .await
            .unwrap();
        assert_eq!(list.data.len(), 3);
    }

    #[tokio::test]
    async fn maps_429_to_rate_limited() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/manga"))
            .respond_with(ResponseTemplate::new(429))
            .mount(&server)
            .await;
        let err = client_with(&server)
            .get::<super::super::response::MangaListResponse>("/manga", &[])
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::RateLimited { .. }));
    }

    #[tokio::test]
    async fn maps_404_to_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/manga/not-here"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let err = client_with(&server)
            .get::<super::super::response::MangaSingleResponse>("/manga/not-here", &[])
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::NotFound { .. }));
    }

    #[tokio::test]
    async fn maps_503_to_provider_down() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/manga"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;
        let err = client_with(&server)
            .get::<super::super::response::MangaListResponse>("/manga", &[])
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::ProviderDown { .. }));
    }

    #[tokio::test]
    async fn maps_invalid_json_to_invalid_response() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/manga"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
            .mount(&server)
            .await;
        let err = client_with(&server)
            .get::<super::super::response::MangaListResponse>("/manga", &[])
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::InvalidResponse { .. }));
    }
}
