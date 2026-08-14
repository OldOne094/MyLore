//! Normalized provider metadata (MISSION-053, ARCHITECTURE §4).
//!
//! The unified shapes every adapter maps its provider JSON into before the
//! domain or UI ever sees it (spec §6). They intentionally mirror the local
//! aggregates (`media`, `content_node`) so a later import/enrich flow is a
//! straight copy; provider-specific shapes never leak past the adapter.

use crate::domain::enums::{ContentType, MediaRelationKind, MediaStatus, NodeKind, PersonRole};
use crate::domain::value_objects::ExternalId;

/// A lightweight search result row (MISSION-059 external search).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderCandidate {
    /// The provider that produced this hit (e.g. `anilist`).
    pub provider: String,
    /// The provider's own id for this title (its external id value).
    pub provider_id: String,
    pub title: String,
    pub content_type: ContentType,
    pub release_year: Option<i32>,
    pub cover_url: Option<String>,
    pub synopsis: Option<String>,
    /// Cross-provider ids surfaced by the search, when available (dedup later).
    pub external_ids: Vec<ExternalId>,
    pub url: Option<String>,
}

/// A person credit on a provider media (author, artist, studio, …).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderPerson {
    pub role: PersonRole,
    pub name: String,
}

/// Full normalized metadata for one title — the `MediaMeta` of the
/// architecture docs. Fields mirror `media` so import/enrich is a copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderMedia {
    pub provider: String,
    pub provider_id: String,
    pub title_main: String,
    pub title_original: Option<String>,
    pub alt_titles: Vec<String>,
    pub content_type: ContentType,
    pub format: Option<String>,
    pub pub_status: MediaStatus,
    pub synopsis: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub release_year: Option<i32>,
    pub language: Option<String>,
    pub country: Option<String>,
    pub content_rating: Option<String>,
    pub pages: Option<u32>,
    pub duration_min: Option<u32>,
    pub ep_count: Option<u32>,
    pub ch_count: Option<u32>,
    pub cover_url: Option<String>,
    pub banner_url: Option<String>,
    pub url: Option<String>,
    pub people: Vec<ProviderPerson>,
    pub genres: Vec<String>,
    pub tags: Vec<String>,
    pub external_ids: Vec<ExternalId>,
}

/// A single node in the provider's content tree (episodes/chapters/volumes).
/// Recursive to mirror `content_node`; adapters that only return flat lists put
/// everything at the top level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderNode {
    pub id: String,
    pub kind: NodeKind,
    pub position: i64,
    pub number: Option<String>,
    pub title: Option<String>,
    pub release_date: Option<String>,
    pub duration_min: Option<i64>,
    pub page_count: Option<i64>,
    pub synopsis: Option<String>,
    pub is_special: bool,
    pub children: Vec<ProviderNode>,
}

/// A directed relation to another title on (possibly) another provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderRelation {
    pub to_provider: String,
    /// The related title's id on `to_provider`.
    pub to_id: String,
    pub relation: MediaRelationKind,
    pub title: Option<String>,
}
