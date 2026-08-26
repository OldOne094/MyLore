//! GCD REST transport (MISSION-109omics).

use crate::domain::provider::error::ProviderError;

use super::PROVIDER_ID;

#[derive(Clone)]
pub struct GcdClient {
    http: reqwest::Client,
    endpoint: String,
}

impl Default for GcdClient {
    fn default() -> Self {
        Self::new()
    }
}

impl GcdClient {
    pub fn new() -> Self {
        let http = reqwest::Client::builder().build().expect("reqwest client builds");
        Self::with_endpoint(http, super::ENDPOINT)
    }

    pub fn with_endpoint(http: reqwest::Client, endpoint: impl Into<String>) -> Self {
        Self {
            http,
            endpoint: endpoint.into(),
        }
    }

    pub async fn search(
        &self,
        query: &str,
    ) -> Result<super::response::SearchResponse, ProviderError> {
        self.get_json(
            &format!("/api/series/?name={}&format=json", urlencoding::encode(query)),
            None::<&str>,
        )
        .await
    }

    pub async fn series(
        &self,
        id: &str,
    ) -> Result<super::response::Series, ProviderError> {
        self.get_json(&format!("/api/series/{id}/"), None::<&str>)
            .await
    }

    async fn get_json<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        _unused: Option<&str>,
    ) -> Result<T, ProviderError> {
        let url = format!("{}{}", self.endpoint, path);
        let response = self
            .http
            .get(&url)
            .header("accept", "application/json")
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
                message: "GCD served a Cloudflare challenge (not retryable)".to_string(),
            });
        }

        serde_json::from_str(&text).map_err(|e| ProviderError::InvalidResponse {
            provider: PROVIDER_ID.to_string(),
            message: format!("invalid GCD payload: {e}"),
        })
    }
}

mod urlencoding {
    pub fn encode(input: &str) -> String {
        let mut out = String::with_capacity(input.len());
        for b in input.bytes() {
            match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    out.push(b as char)
                }
                other => out.push_str(&format!("%{other:02X}")),
            }
        }
        out
    }
}
