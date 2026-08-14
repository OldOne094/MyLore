//! Provider configuration (MISSION-053).
//!
//! Per-provider policy knobs the coordinator applies around every call.
//! `ProviderConfig` is constructed at startup (adapters provide their rate
//! limits from API_PROVIDERS.md; the settings UI in MISSION-063 toggles
//! `enabled` and stores keys via the OS keyring — keys never reach the
//! webview and are never logged).

use std::time::Duration;

use crate::domain::enums::ContentType;

/// Rate-limit policy per provider.
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum request starts per second. `0.0` disables rate limiting
    /// (test doubles, in-memory providers).
    pub requests_per_sec: f64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_sec: 1.0,
        }
    }
}

/// The interval between two allowed request starts for a given rps.
pub fn interval_for(requests_per_sec: f64) -> Duration {
    if requests_per_sec <= 0.0 {
        Duration::ZERO
    } else {
        Duration::from_secs_f64(1.0 / requests_per_sec)
    }
}

/// Everything the coordinator needs to apply policy to one provider.
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    /// Provider id (must match the adapter's `Provider::id`).
    pub id: String,
    /// Off providers are skipped by routing (MISSION-063 toggles this).
    pub enabled: bool,
    /// Domains the provider serves; empty = domain-agnostic. The coordinator
    /// only fans out to providers whose set contains the search's content type
    /// (or to domain-agnostic providers).
    pub content_types: Vec<ContentType>,
    /// Optional static API key (auth models, never logged).
    pub api_key: Option<String>,
    /// Per-request timeout, applied around every attempt.
    pub timeout: Duration,
    /// Retries after the first attempt (0 = no retries).
    pub max_retries: u32,
    /// Base backoff for exponential growth (doubled per retry).
    pub backoff_base: Duration,
    /// Upper bound on any backoff delay (including a server Retry-After).
    pub backoff_max: Duration,
    pub rate_limit: RateLimitConfig,
}

impl ProviderConfig {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            enabled: true,
            content_types: Vec::new(),
            api_key: None,
            timeout: Duration::from_secs(15),
            max_retries: 2,
            backoff_base: Duration::from_millis(200),
            backoff_max: Duration::from_secs(8),
            rate_limit: RateLimitConfig::default(),
        }
    }

    pub fn with_requests_per_sec(mut self, rps: f64) -> Self {
        self.rate_limit.requests_per_sec = rps;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interval_inverts_rps_and_clamps_zero() {
        assert_eq!(interval_for(0.0), Duration::ZERO);
        assert_eq!(interval_for(2.0), Duration::from_millis(500));
        assert_eq!(interval_for(0.5), Duration::from_secs(2));
        assert_eq!(interval_for(-3.0), Duration::ZERO, "negative disables");
    }

    #[test]
    fn config_defaults_are_sane() {
        let config = ProviderConfig::new("anilist").with_requests_per_sec(1.5);
        assert_eq!(config.id, "anilist");
        assert!(config.enabled);
        assert!(config.api_key.is_none());
        assert_eq!(config.max_retries, 2);
        assert_eq!(config.rate_limit.requests_per_sec, 1.5);
    }
}
