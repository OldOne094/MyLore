//! Google Books REST transport (MISSION-058).
//!
//! One GET per call, then typed error mapping. Policy (rate limit, timeout,
//! retry/backoff, cancel) lives in the coordinator above this; the client only
//! maps transport/HTTP/JSON outcomes to `ProviderError`. An API key is appended
//! when set (settings land in MISSION-063 and are stored via the OS keyring —
//! never logged); without one the client still works against mocks.

use serde::de::DeserializeOwned;

use crate::domain::provider::error::ProviderError;

use super::PROVIDER_ID;

/// A polite, identifiable User-Agent.
pub const APP_USER_AGENT: &str = concat!(
    "MyLore/",
    env!("CARGO_PKG_VERSION"),
    " (local-first media tracker)"
);

/// A reqwest-backed Google Books client. `Clone` is cheap (one connection pool).
#[derive(Clone)]
pub struct GoogleBooksClient {
    http: reqwest::Client,
    endpoint: String,
    api_key: Option<String>,
}

impl Default for GoogleBooksClient {
    fn default() -> Self {
        Self::new()
    }
}

impl GoogleBooksClient {
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

    /// Set the Google Books API key. Never logged; MISSION-063 settings wires
    /// this via the OS keyring.
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

    /// GET `endpoint + path` with query params; `api_key` appended when set.
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
            query_pairs.push(("key", key.as_str()));
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
            message: format!("invalid Google Books payload: {e}"),
        })
    }
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::infrastructure::providers::test_support::fixture;

    fn client_with(server: &MockServer) -> GoogleBooksClient {
        GoogleBooksClient::with_endpoint(reqwest::Client::new(), server.uri())
            .with_api_key("test-key")
    }

    #[tokio::test]
    async fn get_appends_api_key_and_parses() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/volumes"))
            .and(query_param("q", "dune"))
            .and(query_param("maxResults", "20"))
            .and(query_param("key", "test-key"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(fixture("googlebooks", "search_volumes.json")),
            )
            .mount(&server)
            .await;

        let out: super::super::response::VolumesResponse = client_with(&server)
            .get("/volumes", &[("q", "dune"), ("maxResults", "20")])
            .await
            .unwrap();
        assert_eq!(out.items.len(), 1);
    }

    #[tokio::test]
    async fn maps_429_to_rate_limited() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/volumes"))
            .respond_with(ResponseTemplate::new(429))
            .mount(&server)
            .await;

        let err = client_with(&server)
            .get::<serde_json::Value>("/volumes", &[])
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            ProviderError::RateLimited { provider, .. } if provider == PROVIDER_ID
        ));
    }

    #[tokio::test]
    async fn maps_401_to_auth_required() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/volumes"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let err = client_with(&server)
            .get::<serde_json::Value>("/volumes", &[])
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::AuthRequired { provider } if provider == PROVIDER_ID));
    }

    #[tokio::test]
    async fn maps_503_to_provider_down() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/volumes"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;

        let err = client_with(&server)
            .get::<serde_json::Value>("/volumes", &[])
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
            .and(path("/volumes"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
            .mount(&server)
            .await;

        let err = client_with(&server)
            .get::<serde_json::Value>("/volumes", &[])
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::InvalidResponse { .. }));
    }
}
