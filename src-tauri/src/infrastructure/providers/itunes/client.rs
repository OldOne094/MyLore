//! iTunes Search REST transport (MISSION-109).

use crate::domain::provider::error::ProviderError;

use super::PROVIDER_ID;

#[derive(Clone)]
pub struct ItunesClient {
    http: reqwest::Client,
    endpoint: String,
}

impl Default for ItunesClient {
    fn default() -> Self {
        Self::new()
    }
}

impl ItunesClient {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .build()
            .expect("reqwest client builds");
        Self::with_endpoint(http, "https://itunes.apple.com")
    }

    /// Test hook: point the client at a local endpoint (wiremock).
    pub fn with_endpoint(http: reqwest::Client, endpoint: impl Into<String>) -> Self {
        Self {
            http,
            endpoint: endpoint.into(),
        }
    }

    pub async fn search(
        &self,
        term: &str,
        media: &str,
        limit: u32,
    ) -> Result<super::response::SearchResponse, ProviderError> {
        let response = self
            .http
            .get(format!("{}/search", self.endpoint))
            .query(&[
                ("term", term),
                ("media", media),
                ("limit", &limit.to_string()),
            ])
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

        response
            .json()
            .await
            .map_err(|e| ProviderError::InvalidResponse {
                provider: PROVIDER_ID.to_string(),
                message: format!("invalid iTunes payload: {e}"),
            })
    }
}
