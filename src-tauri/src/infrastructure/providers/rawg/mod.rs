//! RAWG Video Games Database adapter (MISSION-109).
//!
//! Serves **game** content type via RAWG's REST API (`api.rawg.io`).
//! Requires a free API key from rawg.io (registered by the user in Settings).
//! Key is passed as `?key=` query parameter on every request.
//!
//! Endpoints used:
//! - search:  `GET /api/games?key=X&search=Q&page_size=20`
//! - details: `GET /api/games/{id}?key=X`

pub mod client;
mod normalize;
mod response;

use async_trait::async_trait;

use crate::application::providers::config::ProviderConfig;
use crate::domain::enums::ContentType;
use crate::domain::provider::capabilities::{AuthKind, ProviderCapabilities};
use crate::domain::provider::error::ProviderError;
use crate::domain::provider::types::{ProviderCandidate, ProviderMedia};
use crate::domain::provider::Provider;

pub const PROVIDER_ID: &str = "rawg";

/// Conservative pacing — free tier allows 20k req/month.
pub const REQUESTS_PER_SEC: f64 = 1.0;

pub fn rawg_config() -> ProviderConfig {
    ProviderConfig::new(PROVIDER_ID)
        .with_requests_per_sec(REQUESTS_PER_SEC)
        .with_content_types(vec![ContentType::Game])
}

pub struct RawgProvider {
    client: client::RawgClient,
    caps: ProviderCapabilities,
}

impl RawgProvider {
    pub fn new(client: client::RawgClient) -> Self {
        Self {
            client,
            caps: ProviderCapabilities {
                search: true,
                details: true,
                nodes: false,
                related: false,
                reviews: false,
                images: false,
                seasonal: false,
                auth: AuthKind::Key,
            },
        }
    }
}

#[async_trait]
impl Provider for RawgProvider {
    fn id(&self) -> String {
        PROVIDER_ID.to_string()
    }

    fn name(&self) -> &str {
        "RAWG"
    }

    fn capabilities(&self) -> &ProviderCapabilities {
        &self.caps
    }

    async fn search(
        &self,
        query: &str,
        content_type: Option<ContentType>,
    ) -> Result<Vec<ProviderCandidate>, ProviderError> {
        if content_type.is_some_and(|ct| ct != ContentType::Game) {
            return Ok(Vec::new());
        }
        let Some(key) = self.client.api_key() else {
            return Err(ProviderError::AuthRequired {
                provider: PROVIDER_ID.to_string(),
            });
        };
        let data: response::SearchResponse = self.client.search(query, key, 20).await?;
        Ok(data
            .results
            .iter()
            .filter_map(normalize::candidate)
            .collect())
    }

    async fn get_details(&self, provider_id: &str) -> Result<ProviderMedia, ProviderError> {
        let Some(key) = self.client.api_key() else {
            return Err(ProviderError::AuthRequired {
                provider: PROVIDER_ID.to_string(),
            });
        };
        let data: response::GameDetail = self.client.details(provider_id, key).await?;
        normalize::media(&data).ok_or_else(|| ProviderError::NotFound {
            provider: PROVIDER_ID.to_string(),
        })
    }
}
