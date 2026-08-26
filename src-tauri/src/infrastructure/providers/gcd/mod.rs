//! GCD (Grand Comics Database) adapter — MISSION-109omics.
//!
//! The canonical open comics catalog (2M+ issues, CC-licensed). Search+details
//! live on `www.comics.org` and pass Cloudflare with plain clients (verified).

pub mod client;
mod normalize;
mod response;

use async_trait::async_trait;

use crate::application::providers::config::ProviderConfig;
use crate::domain::enums::ContentType;
use crate::domain::provider::capabilities::{AuthKind, ProviderCapabilities};
use crate::domain::provider::error::ProviderError;
use crate::domain::provider::types::{ProviderCandidate, ProviderMedia, ProviderNode};
use crate::domain::provider::Provider;

pub const PROVIDER_ID: &str = "gcd";
pub const ENDPOINT: &str = "https://www.comics.org";
pub const REQUESTS_PER_SEC: f64 = 2.0;

pub fn gcd_config() -> ProviderConfig {
    ProviderConfig::new(PROVIDER_ID)
        .with_requests_per_sec(REQUESTS_PER_SEC)
        .with_content_types(vec![ContentType::Comic])
}

pub struct GcdProvider {
    client: client::GcdClient,
    caps: ProviderCapabilities,
}

impl GcdProvider {
    pub fn new(client: client::GcdClient) -> Self {
        Self {
            client,
            caps: ProviderCapabilities {
                search: true,
                details: true,
                nodes: true,
                related: false,
                reviews: false,
                images: false,
                seasonal: false,
                auth: AuthKind::None,
            },
        }
    }
}

#[async_trait]
impl Provider for GcdProvider {
    fn id(&self) -> String {
        PROVIDER_ID.to_string()
    }
    fn name(&self) -> &str {
        "Grand Comics Database"
    }
    fn capabilities(&self) -> &ProviderCapabilities {
        &self.caps
    }

    async fn search(
        &self,
        query: &str,
        content_type: Option<ContentType>,
    ) -> Result<Vec<ProviderCandidate>, ProviderError> {
        if content_type.is_some_and(|ct| ct != ContentType::Comic) {
            return Ok(Vec::new());
        }
        let data: response::SearchResponse = self.client.search(query).await?;
        Ok(data
            .results
            .iter()
            .filter_map(normalize::candidate)
            .collect())
    }

    async fn get_details(&self, provider_id: &str) -> Result<ProviderMedia, ProviderError> {
        let series = self.client.series(provider_id).await?;
        normalize::media(&series, provider_id).ok_or_else(|| ProviderError::NotFound {
            provider: PROVIDER_ID.to_string(),
        })
    }

    async fn get_nodes(&self, provider_id: &str) -> Result<Vec<ProviderNode>, ProviderError> {
        let series = self.client.series(provider_id).await?;
        Ok(normalize::nodes(&series))
    }
}
