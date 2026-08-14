//! Provider capabilities (MISSION-053, ARCHITECTURE §4).
//!
//! Each adapter declares what it can do; the app and UI adapt — a search-only
//! provider never claims to enrich nodes (spec §44). The coordinator routes by
//! capability, so a new provider needs nothing more than an honest
//! `capabilities()`.

use crate::domain::enums::ContentType;

/// How a provider authenticates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthKind {
    /// Public data, no key needed.
    None,
    /// A static API key (TMDB, Google Books).
    Key,
    /// OAuth flow (none of the core providers today).
    OAuth,
}

/// The operations a provider can perform, plus its auth model.
///
/// Everything defaults to off; adapters enable exactly what they implement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderCapabilities {
    pub search: bool,
    pub details: bool,
    pub nodes: bool,
    pub related: bool,
    pub reviews: bool,
    pub images: bool,
    pub seasonal: bool,
    pub auth: AuthKind,
}

impl Default for ProviderCapabilities {
    fn default() -> Self {
        Self {
            search: false,
            details: false,
            nodes: false,
            related: false,
            reviews: false,
            images: false,
            seasonal: false,
            auth: AuthKind::None,
        }
    }
}

impl ProviderCapabilities {
    pub fn supports_search(&self) -> bool {
        self.search
    }

    /// The set of content types a provider serves. Providers with an empty set
    /// are domain-agnostic and answer any search. This mirrors the provider
    /// matrix (API_PROVIDERS.md) without hard-coding providers in the domain.
    pub const DOMAIN_AGNOSTIC: &'static [ContentType] = &[];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_all_off_with_no_auth() {
        let caps = ProviderCapabilities::default();
        assert!(!caps.search && !caps.details && !caps.nodes && !caps.related);
        assert!(!caps.reviews && !caps.images && !caps.seasonal);
        assert_eq!(caps.auth, AuthKind::None);
    }

    #[test]
    fn capabilities_are_structurally_enableable() {
        let caps = ProviderCapabilities {
            search: true,
            details: true,
            auth: AuthKind::Key,
            ..Default::default()
        };
        assert!(caps.supports_search());
        assert!(caps.details);
        assert!(!caps.nodes, "unset capabilities stay off");
        assert_eq!(caps.auth, AuthKind::Key);
    }
}
