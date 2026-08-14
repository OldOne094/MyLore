//! The provider interface (MISSION-053, ARCHITECTURE §4).
//!
//! A metadata provider adapter. Adapters are pure normalizers — they map their
//! provider's HTTP/JSON into the unified domain types and typed
//! `ProviderError`s; they never touch the local database. The
//! `ProviderCoordinator` (application layer) owns the policy: rate limiting,
//! timeouts, retry/backoff and cancellation all apply around these methods.
//!
//! Optional operations (`get_nodes`, `get_related`, `get_external_ids`) return
//! `ProviderError::Unsupported` by default; an adapter only overrides what its
//! `capabilities()` claims.

use async_trait::async_trait;

use crate::domain::enums::ContentType;
use crate::domain::provider::capabilities::ProviderCapabilities;
use crate::domain::provider::error::ProviderError;
use crate::domain::provider::types::{
    ProviderCandidate, ProviderMedia, ProviderNode, ProviderRelation,
};
use crate::domain::value_objects::ExternalId;

/// A provider adapter: `Send + Sync` so the coordinator can fan out requests
/// across Tokio tasks.
#[async_trait]
pub trait Provider: Send + Sync {
    /// The stable provider id (e.g. `anilist`, `tmdb`, `openlibrary`).
    fn id(&self) -> String;

    /// A human-readable provider name for the UI (kept in the provider's own
    /// script; i18n lives in the frontend).
    fn name(&self) -> &str;

    /// What this provider can do (routing + UI adaptation).
    fn capabilities(&self) -> &ProviderCapabilities;

    /// Search the provider's catalog. `content_type` narrows the query when the
    /// caller knows the target domain (adapter-specific best effort).
    async fn search(
        &self,
        query: &str,
        content_type: Option<ContentType>,
    ) -> Result<Vec<ProviderCandidate>, ProviderError>;

    /// Full normalized metadata for one provider id.
    async fn get_details(&self, provider_id: &str) -> Result<ProviderMedia, ProviderError>;

    /// The provider's content tree for one id (episodes/chapters/volumes).
    async fn get_nodes(&self, provider_id: &str) -> Result<Vec<ProviderNode>, ProviderError> {
        let _ = provider_id;
        Err(ProviderError::Unsupported {
            provider: self.id(),
            operation: "nodes".to_string(),
        })
    }

    /// Related titles (sequels, prequels, adaptations, …).
    async fn get_related(&self, provider_id: &str) -> Result<Vec<ProviderRelation>, ProviderError> {
        let _ = provider_id;
        Err(ProviderError::Unsupported {
            provider: self.id(),
            operation: "related".to_string(),
        })
    }

    /// The provider's external ids for one id (used for cross-provider dedup).
    async fn get_external_ids(&self, provider_id: &str) -> Result<Vec<ExternalId>, ProviderError> {
        let _ = provider_id;
        Err(ProviderError::Unsupported {
            provider: self.id(),
            operation: "external ids".to_string(),
        })
    }
}
