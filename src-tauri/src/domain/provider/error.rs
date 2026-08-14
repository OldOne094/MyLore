//! Provider errors (MISSION-053, ARCHITECTURE §4).
//!
//! The single error type every provider adapter returns. Adapters map their
//! transport/HTTP results into these variants (via `from_http_status`), and the
//! `ProviderCoordinator` reasons about them generically: `is_retryable` drives
//! retry/backoff, `retry_after` honors the server's Retry-After, and `Display`
//! produces a user-facing message (surfaced by the UI in MISSION-059).

use std::time::Duration;

use thiserror::Error;

/// A provider request that failed in a typed, coordinator-comprehensible way.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProviderError {
    /// The provider asked us to slow down (HTTP 429). `retry_after` carries the
    /// server's Retry-After hint when present.
    #[error("rate limited by {provider}")]
    RateLimited {
        provider: String,
        retry_after: Option<Duration>,
    },

    /// The request exceeded the coordinator's per-call timeout.
    #[error("timed out calling {provider}")]
    Timeout { provider: String },

    /// The provider is down or returned a server error (5xx) / connection error.
    #[error("{provider} is unavailable")]
    ProviderDown {
        provider: String,
        status: Option<u16>,
    },

    /// The requested resource does not exist.
    #[error("not found on {provider}")]
    NotFound { provider: String },

    /// Authentication/authorization failed (HTTP 401/403 or missing key).
    #[error("{provider} requires authentication")]
    AuthRequired { provider: String },

    /// Some other non-retryable HTTP status.
    #[error("{provider} returned HTTP {status}")]
    Http { provider: String, status: u16 },

    /// The provider returned data that could not be normalized into the domain.
    #[error("{provider} returned unreadable data: {message}")]
    InvalidResponse { provider: String, message: String },

    /// The operation was canceled by the caller.
    #[error("request canceled")]
    Canceled { provider: String },

    /// The provider does not implement the requested capability/operation.
    #[error("{provider} does not support {operation}")]
    Unsupported { provider: String, operation: String },

    /// A transport-level failure (connect, DNS, I/O) — retryable.
    #[error("transport error from {provider}: {message}")]
    Transport { provider: String, message: String },
}

impl ProviderError {
    /// Map an HTTP status to the typed error, ready for `is_retryable`/
    /// coordinator policy. `404` and `401/403` are deliberately distinct so a
    /// retry never re-hits a deterministic miss.
    pub fn from_http_status(provider: impl Into<String>, status: u16) -> Self {
        let provider = provider.into();
        match status {
            429 => Self::RateLimited {
                provider,
                retry_after: None,
            },
            401 | 403 => Self::AuthRequired { provider },
            404 => Self::NotFound { provider },
            500..=599 => Self::ProviderDown {
                provider,
                status: Some(status),
            },
            _ => Self::Http { provider, status },
        }
    }

    /// Whether the coordinator should retry this error. Rate limits, timeouts,
    /// server errors and transport failures are transient; auth, not-found and
    /// invalid responses are deterministic and must not be retried.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::RateLimited { .. }
                | Self::Timeout { .. }
                | Self::ProviderDown { .. }
                | Self::Transport { .. }
        )
    }

    /// The server-requested delay before retrying, when the provider said so.
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::RateLimited { retry_after, .. } => *retry_after,
            _ => None,
        }
    }

    fn provider(&self) -> &str {
        match self {
            Self::RateLimited { provider, .. }
            | Self::Timeout { provider }
            | Self::ProviderDown { provider, .. }
            | Self::NotFound { provider }
            | Self::AuthRequired { provider }
            | Self::Http { provider, .. }
            | Self::InvalidResponse { provider, .. }
            | Self::Canceled { provider }
            | Self::Unsupported { provider, .. }
            | Self::Transport { provider, .. } => provider,
        }
    }
}

/// Convenience accessor so call sites don't pattern-match to name a provider.
impl ProviderError {
    pub fn provider_name(&self) -> &str {
        self.provider()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_status_mapping_is_typed() {
        assert!(matches!(
            ProviderError::from_http_status("anilist", 429),
            ProviderError::RateLimited { .. }
        ));
        assert!(matches!(
            ProviderError::from_http_status("tmdb", 401),
            ProviderError::AuthRequired { .. }
        ));
        assert!(matches!(
            ProviderError::from_http_status("tmdb", 403),
            ProviderError::AuthRequired { .. }
        ));
        assert!(matches!(
            ProviderError::from_http_status("openlibrary", 404),
            ProviderError::NotFound { .. }
        ));
        assert!(matches!(
            ProviderError::from_http_status("mangadex", 503),
            ProviderError::ProviderDown {
                status: Some(503),
                ..
            }
        ));
        assert!(matches!(
            ProviderError::from_http_status("mangadex", 418),
            ProviderError::Http { status: 418, .. }
        ));
    }

    #[test]
    fn retryable_classification() {
        let p = |name: &str| name.to_string();
        assert!(ProviderError::RateLimited {
            provider: p("x"),
            retry_after: None,
        }
        .is_retryable());
        assert!(ProviderError::Timeout { provider: p("x") }.is_retryable());
        assert!(ProviderError::ProviderDown {
            provider: p("x"),
            status: Some(500),
        }
        .is_retryable());
        assert!(ProviderError::Transport {
            provider: p("x"),
            message: "connect".into(),
        }
        .is_retryable());

        assert!(!ProviderError::NotFound { provider: p("x") }.is_retryable());
        assert!(!ProviderError::AuthRequired { provider: p("x") }.is_retryable());
        assert!(!ProviderError::Http {
            provider: p("x"),
            status: 400,
        }
        .is_retryable());
        assert!(!ProviderError::InvalidResponse {
            provider: p("x"),
            message: "bad".into(),
        }
        .is_retryable());
        assert!(!ProviderError::Canceled { provider: p("x") }.is_retryable());
        assert!(!ProviderError::Unsupported {
            provider: p("x"),
            operation: "nodes".into(),
        }
        .is_retryable());
    }

    #[test]
    fn retry_after_carries_server_hint() {
        let err = ProviderError::RateLimited {
            provider: "anilist".into(),
            retry_after: Some(Duration::from_secs(7)),
        };
        assert_eq!(err.retry_after(), Some(Duration::from_secs(7)));
        assert_eq!(err.provider_name(), "anilist");
        assert_eq!(
            ProviderError::Timeout {
                provider: "x".into()
            }
            .retry_after(),
            None
        );
    }

    #[test]
    fn display_messages_are_readable() {
        assert_eq!(
            ProviderError::from_http_status("tmdb", 404).to_string(),
            "not found on tmdb"
        );
        let _ = format!(
            "{}",
            ProviderError::Transport {
                provider: "x".into(),
                message: "dns".into(),
            }
        );
    }
}
