//! iTunes Search adapter (MISSION-109).
//!
//! Serves **podcast** and **music** content types via Apple's public,
//! keyless iTunes Search API (`itunes.apple.com/search`). One GET per call.
//! All policy (rate limit, timeout, retry/backoff, cancel) lives in the
//! coordinator above this; the client maps transport/HTTP outcomes to
//! typed errors.

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

pub const PROVIDER_ID: &str = "itunes";

/// Conservative pacing for a public search API.
pub const REQUESTS_PER_SEC: f64 = 3.0;

/// The config the coordinator registers iTunes Search with. It serves both
/// podcast and music content types from a single adapter.
pub fn itunes_config() -> ProviderConfig {
    ProviderConfig::new(PROVIDER_ID)
        .with_requests_per_sec(REQUESTS_PER_SEC)
        .with_content_types(vec![ContentType::Podcast, ContentType::Music])
}

pub struct ItunesProvider {
    client: client::ItunesClient,
    caps: ProviderCapabilities,
}

impl ItunesProvider {
    pub fn new(client: client::ItunesClient) -> Self {
        Self {
            client,
            caps: ProviderCapabilities {
                search: true,
                details: false,
                nodes: false,
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
impl Provider for ItunesProvider {
    fn id(&self) -> String {
        PROVIDER_ID.to_string()
    }

    fn name(&self) -> &str {
        "iTunes"
    }

    fn capabilities(&self) -> &ProviderCapabilities {
        &self.caps
    }

    async fn search(
        &self,
        query: &str,
        content_type: Option<ContentType>,
    ) -> Result<Vec<ProviderCandidate>, ProviderError> {
        let media = match content_type {
            Some(ContentType::Podcast) => "podcast",
            Some(ContentType::Music) => "music",
            Some(_) => return Ok(Vec::new()),
            None => return Ok(Vec::new()),
        };
        let data: response::SearchResponse = self.client.search(query, media, 20).await?;
        let ct = content_type.unwrap_or(ContentType::Podcast);
        Ok(data
            .results
            .iter()
            .filter_map(|item| {
                let mut c = normalize::candidate(item)?;
                c.content_type = ct;
                Some(c)
            })
            .collect())
    }

    async fn get_details(&self, _provider_id: &str) -> Result<ProviderMedia, ProviderError> {
        Err(ProviderError::Unsupported {
            provider: PROVIDER_ID.to_string(),
            operation: "details".into(),
        })
    }
}
