//! Hardcover GraphQL transport (MISSION-064).
//!
//! Thin: one POST per call, then typed error mapping. All policy (timeout,
//! retry/backoff, rate limit, cancel) lives in the coordinator above this; the
//! client just maps transport/HTTP/GraphQL outcomes to `ProviderError`. A Bearer
//! token (settings land in MISSION-063 and are stored via the OS keyring —
//! never logged) is sent when set; without one the client still works against
//! mocks, and real requests surface `AuthRequired` on a 401/403.

use serde::de::DeserializeOwned;
use serde_json::{json, Value};

use crate::domain::provider::error::ProviderError;

use super::response::Envelope;
use super::PROVIDER_ID;

/// A polite, identifiable User-Agent.
pub const APP_USER_AGENT: &str = concat!(
    "MyLore/",
    env!("CARGO_PKG_VERSION"),
    " (local-first media tracker)"
);

/// A reqwest-backed Hardcover client. `Clone` is cheap (one connection pool).
#[derive(Clone)]
pub struct HardcoverClient {
    http: reqwest::Client,
    endpoint: String,
    token: Option<String>,
}

impl Default for HardcoverClient {
    fn default() -> Self {
        Self::new()
    }
}

impl HardcoverClient {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .user_agent(APP_USER_AGENT)
            .build()
            .expect("reqwest client builds");
        Self {
            http,
            endpoint: super::ENDPOINT.to_string(),
            token: None,
        }
    }

    /// Set the Hardcover API token (sent as `Authorization: Bearer <token>`).
    /// Never logged; MISSION-063 settings wires this via the OS keyring.
    pub fn with_api_key(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    /// Test hook: point the client at a local endpoint (wiremock).
    pub fn with_endpoint(http: reqwest::Client, endpoint: impl Into<String>) -> Self {
        Self {
            http,
            endpoint: endpoint.into(),
            token: None,
        }
    }

    /// Execute a GraphQL query, returning the parsed `data` payload. Maps
    /// transport failures → `Transport`, HTTP status → `from_http_status`
    /// (401/403→AuthRequired, 429→RateLimited, 404→NotFound, 5xx→ProviderDown),
    /// GraphQL `errors`/malformed envelopes → `InvalidResponse`.
    pub async fn graphql<T: DeserializeOwned>(
        &self,
        query: &str,
        variables: Value,
    ) -> Result<T, ProviderError> {
        let body = json!({ "query": query, "variables": variables });
        let mut request = self.http.post(&self.endpoint).json(&body);
        if let Some(token) = &self.token {
            request = request.header("authorization", format!("Bearer {token}"));
        }
        let response = request.send().await.map_err(|e| ProviderError::Transport {
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

        let envelope: Envelope<T> =
            serde_json::from_str(&text).map_err(|e| ProviderError::InvalidResponse {
                provider: PROVIDER_ID.to_string(),
                message: format!("invalid GraphQL envelope: {e}"),
            })?;

        if let Some(errors) = envelope.errors {
            let message = errors
                .iter()
                .filter_map(|e| e.message.clone())
                .collect::<Vec<_>>()
                .join("; ");
            return Err(ProviderError::InvalidResponse {
                provider: PROVIDER_ID.to_string(),
                message: if message.is_empty() {
                    "GraphQL error".to_string()
                } else {
                    message
                },
            });
        }

        envelope.data.ok_or_else(|| ProviderError::InvalidResponse {
            provider: PROVIDER_ID.to_string(),
            message: "missing data in GraphQL response".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use wiremock::matchers::{body_partial_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::infrastructure::providers::hardcover::graphql;
    use crate::infrastructure::providers::test_support::hardcover_fixture;

    #[tokio::test]
    async fn posts_query_and_sends_bearer_token() {
        let server = MockServer::start().await;
        let client = HardcoverClient::with_endpoint(reqwest::Client::new(), server.uri())
            .with_api_key("secret-token");

        Mock::given(method("POST"))
            .and(path("/"))
            .and(header("authorization", "Bearer secret-token"))
            .and(body_partial_json(json!({ "query": graphql::SEARCH_QUERY })))
            .and(body_partial_json(
                json!({ "variables": { "query": "dune" } }),
            ))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(hardcover_fixture("search_books.json")),
            )
            .mount(&server)
            .await;

        let data: super::super::response::SearchPayload = client
            .graphql(graphql::SEARCH_QUERY, json!({ "query": "dune" }))
            .await
            .unwrap();
        assert_eq!(data.search.results.len(), 3);
    }

    #[tokio::test]
    async fn sends_no_auth_header_without_token() {
        let server = MockServer::start().await;
        let client = HardcoverClient::with_endpoint(reqwest::Client::new(), server.uri());

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(hardcover_fixture("search_books.json")),
            )
            .mount(&server)
            .await;

        let data: super::super::response::SearchPayload = client
            .graphql(graphql::SEARCH_QUERY, json!({ "query": "dune" }))
            .await
            .unwrap();
        assert_eq!(data.search.results.len(), 3);
    }

    #[tokio::test]
    async fn maps_http_401_to_auth_required() {
        let server = MockServer::start().await;
        let client = HardcoverClient::with_endpoint(reqwest::Client::new(), server.uri());
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let err = client
            .graphql::<super::super::response::SearchPayload>(graphql::SEARCH_QUERY, json!({}))
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::AuthRequired { .. }));
    }

    #[tokio::test]
    async fn maps_http_429_to_rate_limited() {
        let server = MockServer::start().await;
        let client = HardcoverClient::with_endpoint(reqwest::Client::new(), server.uri());
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(429))
            .mount(&server)
            .await;

        let err = client
            .graphql::<super::super::response::SearchPayload>(graphql::SEARCH_QUERY, json!({}))
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::RateLimited { .. }));
    }

    #[tokio::test]
    async fn maps_http_503_to_provider_down() {
        let server = MockServer::start().await;
        let client = HardcoverClient::with_endpoint(reqwest::Client::new(), server.uri());
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;

        let err = client
            .graphql::<super::super::response::SearchPayload>(graphql::SEARCH_QUERY, json!({}))
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::ProviderDown { .. }));
    }

    #[tokio::test]
    async fn maps_graphql_errors_to_invalid_response() {
        let server = MockServer::start().await;
        let client = HardcoverClient::with_endpoint(reqwest::Client::new(), server.uri());
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"errors":[{"message":"field not found"}]}"#),
            )
            .mount(&server)
            .await;

        let err = client
            .graphql::<super::super::response::SearchPayload>(graphql::SEARCH_QUERY, json!({}))
            .await
            .unwrap_err();
        match err {
            ProviderError::InvalidResponse { message, .. } => {
                assert!(message.contains("field not found"))
            }
            other => panic!("expected InvalidResponse, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn maps_invalid_json_to_invalid_response() {
        let server = MockServer::start().await;
        let client = HardcoverClient::with_endpoint(reqwest::Client::new(), server.uri());
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
            .mount(&server)
            .await;

        let err = client
            .graphql::<super::super::response::SearchPayload>(graphql::SEARCH_QUERY, json!({}))
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::InvalidResponse { .. }));
    }
}
