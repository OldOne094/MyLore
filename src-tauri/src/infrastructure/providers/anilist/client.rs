//! AniList GraphQL transport (MISSION-054).
//!
//! Thin: one POST per call, then typed error mapping. All policy (timeout,
//! retry/backoff, rate limit, cancel) lives in the coordinator above this; the
//! client just maps transport/HTTP/GraphQL outcomes to `ProviderError`.

use serde::de::DeserializeOwned;
use serde_json::{json, Value};

use crate::domain::provider::error::ProviderError;

use super::response::Envelope;
use super::PROVIDER_ID;

/// A polite, identifiable User-Agent (good citizenship for public APIs).
pub const APP_USER_AGENT: &str = concat!(
    "MyLore/",
    env!("CARGO_PKG_VERSION"),
    " (local-first media tracker)"
);

/// A reqwest-backed AniList client. `Clone` is cheap (one connection pool).
#[derive(Clone)]
pub struct AniListClient {
    http: reqwest::Client,
    endpoint: String,
}

impl Default for AniListClient {
    fn default() -> Self {
        Self::new()
    }
}

impl AniListClient {
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

    /// Execute a GraphQL query, returning the parsed `data` payload. Maps
    /// transport failures → `Transport`, HTTP status → `from_http_status`,
    /// GraphQL `errors`/malformed envelopes → `InvalidResponse`.
    pub async fn graphql<T: DeserializeOwned>(
        &self,
        query: &str,
        variables: Value,
    ) -> Result<T, ProviderError> {
        let body = json!({ "query": query, "variables": variables });
        let response = self
            .http
            .post(&self.endpoint)
            .json(&body)
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
    use wiremock::matchers::{body_partial_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::infrastructure::providers::anilist::graphql;
    use crate::infrastructure::providers::test_support::fixture;

    #[tokio::test]
    async fn posts_query_and_variables_and_parses_data() {
        let server = MockServer::start().await;
        let client = AniListClient::with_endpoint(reqwest::Client::new(), server.uri());

        Mock::given(method("POST"))
            .and(path("/"))
            .and(body_partial_json(json!({ "query": graphql::SEARCH_QUERY })))
            .and(body_partial_json(
                json!({ "variables": { "q": "bebop", "type": null } }),
            ))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(fixture("anilist", "search_anime.json")),
            )
            .mount(&server)
            .await;

        let data: super::super::response::SearchData = client
            .graphql(graphql::SEARCH_QUERY, json!({ "q": "bebop", "type": null }))
            .await
            .unwrap();
        assert_eq!(data.page.media.len(), 2);
        assert_eq!(data.page.media[0].id, 1);
    }

    #[tokio::test]
    async fn maps_http_429_to_rate_limited() {
        let server = MockServer::start().await;
        let client = AniListClient::with_endpoint(reqwest::Client::new(), server.uri());
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(429))
            .mount(&server)
            .await;
        let err = client
            .graphql::<super::super::response::SearchData>(graphql::SEARCH_QUERY, json!({}))
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::RateLimited { .. }));
    }

    #[tokio::test]
    async fn maps_http_500_to_provider_down() {
        let server = MockServer::start().await;
        let client = AniListClient::with_endpoint(reqwest::Client::new(), server.uri());
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;
        let err = client
            .graphql::<super::super::response::SearchData>(graphql::SEARCH_QUERY, json!({}))
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::ProviderDown { .. }));
    }

    #[tokio::test]
    async fn maps_graphql_errors_to_invalid_response() {
        let server = MockServer::start().await;
        let client = AniListClient::with_endpoint(reqwest::Client::new(), server.uri());
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"errors":[{"message":"Invalid id"}]}"#),
            )
            .mount(&server)
            .await;
        let err = client
            .graphql::<super::super::response::SearchData>(graphql::SEARCH_QUERY, json!({}))
            .await
            .unwrap_err();
        match err {
            ProviderError::InvalidResponse { message, .. } => {
                assert!(message.contains("Invalid id"))
            }
            other => panic!("expected InvalidResponse, got {other:?}"),
        }
    }
}
