//! WTR-LAB REST transport (MISSION-131).
//!
//! Plain reqwest — live-verified that this zone's Cloudflare passes normal
//! clients. JSON and HTML GET helpers share one error mapping; an active
//! challenge page is reported as a non-retryable `InvalidResponse` (the
//! coordinator's retry loop must not hammer it).

use serde::de::DeserializeOwned;

use crate::domain::provider::error::ProviderError;

use super::PROVIDER_ID;

pub const APP_USER_AGENT: &str = concat!(
    "MyLore/",
    env!("CARGO_PKG_VERSION"),
    " (local-first media tracker; WTR-LAB series metadata)"
);

#[derive(Clone)]
pub struct WtrLabClient {
    http: reqwest::Client,
    endpoint: String,
}

impl Default for WtrLabClient {
    fn default() -> Self {
        Self::new()
    }
}

impl WtrLabClient {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .user_agent(APP_USER_AGENT)
            .build()
            .expect("reqwest client builds");
        Self::with_endpoint(http, super::ENDPOINT)
    }

    /// Test hook: point the client at a local endpoint (wiremock).
    pub fn with_endpoint(http: reqwest::Client, endpoint: impl Into<String>) -> Self {
        Self {
            http,
            endpoint: endpoint.into(),
        }
    }

    async fn get_text(&self, path: &str, params: &[(&str, &str)]) -> Result<String, ProviderError> {
        let response = self
            .http
            .get(format!("{}{}", self.endpoint, path))
            .query(params)
            .header("accept", "text/html,application/json;q=0.9,*/*;q=0.8")
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

        if text.contains("Just a moment") && text.contains("cf-turnstile") {
            return Err(ProviderError::InvalidResponse {
                provider: PROVIDER_ID.to_string(),
                message: "WTR-LAB served a Cloudflare challenge (not retryable)".to_string(),
            });
        }
        Ok(text)
    }

    /// GET a JSON API route.
    pub async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, &str)],
    ) -> Result<T, ProviderError> {
        let text = self.get_text(path, params).await?;
        serde_json::from_str(&text).map_err(|e| ProviderError::InvalidResponse {
            provider: PROVIDER_ID.to_string(),
            message: format!("invalid WTR-LAB payload: {e}"),
        })
    }

    /// GET an HTML page.
    pub async fn get_html(&self, path: &str) -> Result<String, ProviderError> {
        self.get_text(path, &[]).await
    }
}
