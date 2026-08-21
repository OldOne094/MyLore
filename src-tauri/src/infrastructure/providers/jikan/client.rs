//! Jikan REST transport (MISSION-058).
//!
//! One GET per call, then typed error mapping. Policy (rate limit, timeout,
//! retry/backoff, cancel) lives in the coordinator above this; the client only
//! maps transport/HTTP/JSON outcomes to `ProviderError`. Jikan needs no auth.

use serde::de::DeserializeOwned;

use crate::domain::provider::error::ProviderError;

use super::PROVIDER_ID;

/// A polite, identifiable User-Agent.
pub const APP_USER_AGENT: &str = concat!(
    "MyLore/",
    env!("CARGO_PKG_VERSION"),
    " (local-first media tracker)"
);

/// A reqwest-backed Jikan client. `Clone` is cheap (one connection pool).
#[derive(Clone)]
pub struct JikanClient {
    http: reqwest::Client,
    endpoint: String,
}

impl Default for JikanClient {
    fn default() -> Self {
        Self::new()
    }
}

impl JikanClient {
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

    /// GET `endpoint + path` with query params. Maps transport failures →
    /// `Transport`, HTTP status → `from_http_status` (404→NotFound, 429→
    /// RateLimited, 5xx→ProviderDown) and malformed JSON → `InvalidResponse`.
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
            message: format!("invalid Jikan payload: {e}"),
        })
    }
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::infrastructure::providers::test_support::fixture;

    fn client_with(server: &MockServer) -> JikanClient {
        JikanClient::with_endpoint(reqwest::Client::new(), server.uri())
    }

    #[tokio::test]
    async fn get_sends_query_and_parses() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/anime"))
            .and(query_param("q", "hxh"))
            .and(query_param("limit", "20"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(fixture("jikan", "search_anime.json")),
            )
            .mount(&server)
            .await;

        let out: super::super::response::AnimeSearchResponse = client_with(&server)
            .get("/anime", &[("q", "hxh"), ("limit", "20")])
            .await
            .unwrap();
        assert_eq!(out.data.len(), 2);
    }

    #[tokio::test]
    async fn maps_429_to_rate_limited() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/anime"))
            .respond_with(ResponseTemplate::new(429))
            .mount(&server)
            .await;

        let err = client_with(&server)
            .get::<serde_json::Value>("/anime", &[])
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            ProviderError::RateLimited { provider, .. } if provider == PROVIDER_ID
        ));
    }

    #[tokio::test]
    async fn maps_404_to_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/anime/999999"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let err = client_with(&server)
            .get::<serde_json::Value>("/anime/999999", &[])
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::NotFound { provider } if provider == PROVIDER_ID));
    }

    #[tokio::test]
    async fn maps_503_to_provider_down() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/anime"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;

        let err = client_with(&server)
            .get::<serde_json::Value>("/anime", &[])
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            ProviderError::ProviderDown { provider, .. } if provider == PROVIDER_ID
        ));
    }

    #[tokio::test]
    async fn maps_invalid_json() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/anime"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
            .mount(&server)
            .await;

        let err = client_with(&server)
            .get::<serde_json::Value>("/anime", &[])
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::InvalidResponse { .. }));
    }
}
