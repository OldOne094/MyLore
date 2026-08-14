//! Provider domain (MISSION-053, ARCHITECTURE §4, ADR-004).
//!
//! The capability-based provider seam: the `Provider` trait every adapter
//! implements, the `ProviderCapabilities` it declares, the unified metadata
//! types adapters normalize into, and the typed `ProviderError` map the
//! coordinator reasons about. Pure and side-effect-free — no HTTP, no SQL.
//! Policy (rate limit / retry / timeout / cancel) lives in the application
//! layer (`application::providers::coordinator`).

pub mod capabilities;
pub mod error;
pub mod trait_;
pub mod types;

pub use capabilities::{AuthKind, ProviderCapabilities};
pub use error::ProviderError;
pub use trait_::Provider;
pub use types::{ProviderCandidate, ProviderMedia, ProviderNode, ProviderPerson, ProviderRelation};
