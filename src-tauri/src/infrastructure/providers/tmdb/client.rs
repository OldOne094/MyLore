//! TMDB REST transport (MISSION-055).
//!
//! One GET per call, then typed error mapping. Policy (rate limit, timeout,
//! retry/backoff, cancel) lives in the coordinator above this; the client only
//! maps transport/HTTP/JSON outcomes to `ProviderError`. The API key is
//! appended as a query param when set (settings land in MISSION-063 and are
//! stored via the OS keyring — never logged).

use serde::de::DeserializeOwned;

use crate::domain::provider::error::ProviderError;

use super::PROVIDER_ID;

/// A polite, identifiable User-Agent (TMDB requires attribution-friendly apps).
pub const APP_USER_AGENT: &str = concat!(
    "MyLore/",
    env!("CARGO_PKG_VERSION"),
    " (local-first media tracker; TMDB attribution required)"
);

/// A reqwest-backed TMDB client. `Clone` is cheap (one connection pool).
#[derive(Clone)]
pub struct TmdbClient {
    http: reqwest::Client,
    endpoint: String,
    api_key: Option<String>,
}

impl Default for TmdbClient {
    fn default() -> Self {
        Self::new()
    }
}

impl TmdbClient {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .user_agent(APP_USER_AGENT)
            .build()
            .expect("reqwest client builds");
        Self {
            http,
            endpoint: super::ENDPOINT.to_string(),
            api_key: None,
        }
    }

    /// Set the TMDB API key. Never logged; MISSION-063 settings wires this via
    /// the OS keyring.
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Test hook: point the client at a local endpoint (wiremock).
    pub fn with_endpoint(http: reqwest::Client, endpoint: impl Into<String>) -> Self {
        Self {
            http,
            endpoint: endpoint.into(),
            api_key: None,
        }
    }

    /// GET `endpoint + path` with query params; `api_key` is appended when set.
    /// Maps transport failures → `Transport`, HTTP status → `from_http_status`
    /// (404→NotFound, 401/403→AuthRequired, 429→RateLimited, 5xx→ProviderDown)
    /// and malformed JSON → `InvalidResponse`.
    pub async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, &str)],
    ) -> Result<T, ProviderError> {
        let mut query_pairs: Vec<(&str, &str)> = params.to_vec();
        if let Some(key) = &self.api_key {
            query_pairs.push(("api_key", key.as_str()));
        }
        let response = self
            .http
            .get(format!("{}{}", self.endpoint, path))
            .query(&query_pairs)
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
            message: format!("invalid TMDB payload: {e}"),
        })
    }
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::infrastructure::providers::test_support::tmdb_fixture;

    fn client_with(server: &MockServer) -> TmdbClient {
        TmdbClient::with_endpoint(reqwest::Client::new(), server.uri()).with_api_key("test-key")
    }

    #[tokio::test]
    async fn get_appends_api_key_and_parses() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/movie/603/external_ids"))
            .and(query_param("api_key", "test-key"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(tmdb_fixture("external_ids_movie.json")),
            )
            .mount(&server)
            .await;
        let ext: super::super::response::ExternalIds = client_with(&server)
            .get("/movie/603/external_ids", &[])
            .await
            .unwrap();
        assert_eq!(ext.imdb_id.as_deref(), Some("tt0133093"));
    }

    #[tokio::test]
    async fn maps_429_to_rate_limited() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/search/multi"))
            .respond_with(ResponseTemplate::new(429))
            .mount(&server)
            .await;
        let err = client_with(&server)
            .get::<super::super::response::SearchResponse>("/search/multi", &[])
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::RateLimited { .. }));
    }

    #[tokio::test]
    async fn maps_401_to_auth_required() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/movie/1"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        let err = client_with(&server)
            .get::<super::super::response::MediaDetails>("/movie/1", &[])
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::AuthRequired { .. }));
    }

    #[tokio::test]
    async fn maps_404_to_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/movie/999"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let err = client_with(&server)
            .get::<super::super::response::MediaDetails>("/movie/999", &[])
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::NotFound { .. }));
    }

    #[tokio::test]
    async fn maps_503_to_provider_down() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/movie/1"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;
        let err = client_with(&server)
            .get::<super::super::response::MediaDetails>("/movie/1", &[])
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::ProviderDown { .. }));
    }

    #[tokio::test]
    async fn maps_invalid_json_to_invalid_response() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/movie/1"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
            .mount(&server)
            .await;
        let err = client_with(&server)
            .get::<super::super::response::MediaDetails>("/movie/1", &[])
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::InvalidResponse { .. }));
    }
}
