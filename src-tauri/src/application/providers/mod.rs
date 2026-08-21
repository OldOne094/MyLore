//! Provider application layer (MISSION-053, ARCHITECTURE §4).
//!
//! The `ProviderCoordinator` — the policy engine that wraps every provider call
//! with rate limiting, timeout, retry/backoff + jitter, cancellation and typed
//! error mapping. It is deliberately HTTP-agnostic (works over the `Provider`
//! trait), so the whole policy layer is unit-testable with fake adapters and
//! paused time; the transport adapters land with their milestones (M8).

pub mod config;
pub mod coordinator;
pub mod rate_limiter;
pub mod settings;

pub use config::{interval_for, ProviderConfig, RateLimitConfig};
pub use coordinator::{
    CancellationToken, ProviderCoordinator, ProviderInfo, SearchFailure, SearchHit, SearchOutcome,
};
pub use rate_limiter::RateLimiter;
pub use settings::{EntryBuilder, ProviderSettingsService, ProviderSettingsView, ProviderTestView};
