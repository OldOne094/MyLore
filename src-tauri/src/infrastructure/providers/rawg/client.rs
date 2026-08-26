//! RAWG REST transport (MISSION-109).

use serde::de::DeserializeOwned;

use crate::domain::provider::error::ProviderError;

use super::PROVIDER_ID;

#[derive(Clone)]
pub struct RawgClient {
    http: reqwest::Client,
    endpoint: String,
    api_key: Option<String>,
}

impl Default for RawgClient {
    fn default() -> Self {
        Self::new()
    }
}

impl RawgClient {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .build()
            .expect("reqwest client builds");
        Self::with_endpoint(http, "https://api.rawg.io")
    }

    /// Set the RAWG API key (from the settings UI).
    pub fn with_api_key(mut self, key: &str) -> Self {
        self.api_key = Some(key.to_string());
        self
    }

    pub fn with_endpoint(http: reqwest::Client, endpoint: impl Into<String>) -> Self {
        Self {
            http,
            endpoint: endpoint.into(),
            api_key: None,
        }
    }

    /// The API key for this client, if set.
    pub fn api_key(&self) -> Option<&str> {
        self.api_key.as_deref()
    }

    async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, &str)],
    ) -> Result<T, ProviderError> {
        let url = format!("{}{}", self.endpoint, path);
        let mut request = self.http.get(&url).query(params);
        if let Some(key) = &self.api_key {
            request = request.query(&[("key", key)]);
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

        response
            .json()
            .await
            .map_err(|e| ProviderError::InvalidResponse {
                provider: PROVIDER_ID.to_string(),
                message: format!("invalid RAWG payload: {e}"),
            })
    }

    pub async fn search(
        &self,
        query: &str,
        _key: &str,
        limit: u32,
    ) -> Result<super::response::SearchResponse, ProviderError> {
        self.get(
            "/api/games",
            &[("search", query), ("page_size", &limit.to_string())],
        )
        .await
    }

    pub async fn details(
        &self,
        id: &str,
        _key: &str,
    ) -> Result<super::response::GameDetail, ProviderError> {
        self.get(&format!("/api/games/{id}"), &[]).await
    }
}
