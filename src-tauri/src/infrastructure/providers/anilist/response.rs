//! Serde models for AniList GraphQL responses (MISSION-054).
//!
//! These mirror the exact AniList shapes (camelCase) so normalization is a
//! pure mapping. `Envelope<T>` is the GraphQL wrapper: `data` holds the typed
//! payload, `errors` carries GraphQL-level failures.

use serde::Deserialize;

/// The GraphQL envelope: `{ data?, errors? }`.
#[derive(Debug, Deserialize)]
pub(crate) struct Envelope<T> {
    pub data: Option<T>,
    pub errors: Option<Vec<GraphError>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GraphError {
    pub message: Option<String>,
}

/// `data.Page` for `search`.
#[derive(Debug, Deserialize)]
pub(crate) struct SearchData {
    #[serde(rename = "Page")]
    pub page: SearchPage,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SearchPage {
    pub media: Vec<MediaSearch>,
}

/// `data.Media` for `get_details`/`get_nodes`/`get_related`/`get_external_ids`.
#[derive(Debug, Deserialize)]
pub(crate) struct DetailsData {
    #[serde(rename = "Media")]
    pub media: Option<MediaFull>,
}

/// A search-result row (a subset of the full Media shape).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MediaSearch {
    pub id: i64,
    #[serde(rename = "type")]
    pub media_type: Option<String>,
    pub format: Option<String>,
    pub country_of_origin: Option<String>,
    pub title: Title,
    pub cover_image: Option<CoverImage>,
    pub description: Option<String>,
    pub start_date: Option<FuzzyDate>,
    pub site_url: Option<String>,
}

/// The full Media shape used by every detail operation.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MediaFull {
    pub id: i64,
    #[serde(rename = "type")]
    pub media_type: Option<String>,
    pub format: Option<String>,
    pub country_of_origin: Option<String>,
    pub title: Title,
    pub cover_image: Option<CoverImage>,
    pub banner_image: Option<String>,
    pub description: Option<String>,
    pub start_date: Option<FuzzyDate>,
    pub end_date: Option<FuzzyDate>,
    pub status: Option<String>,
    pub episodes: Option<i64>,
    pub chapters: Option<i64>,
    pub duration: Option<i64>,
    pub genres: Option<Vec<String>>,
    pub tags: Option<Vec<Tag>>,
    pub studios: Option<StudioConnection>,
    pub staff: Option<StaffConnection>,
    pub relations: Option<RelationConnection>,
    pub external_links: Option<Vec<ExternalLink>>,
    pub site_url: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Title {
    pub romaji: Option<String>,
    pub english: Option<String>,
    pub native: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FuzzyDate {
    pub year: Option<i64>,
    pub month: Option<i64>,
    pub day: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CoverImage {
    pub extra_large: Option<String>,
    pub large: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Tag {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioConnection {
    pub edges: Option<Vec<StudioEdge>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioEdge {
    pub is_main: Option<bool>,
    pub node: StudioNode,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioNode {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StaffConnection {
    pub edges: Option<Vec<StaffEdge>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StaffEdge {
    pub role: Option<String>,
    pub node: StaffNode,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StaffNode {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RelationConnection {
    pub edges: Option<Vec<RelationEdge>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RelationEdge {
    pub relation_type: Option<String>,
    pub node: RelationNode,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RelationNode {
    pub id: i64,
    pub title: Title,
}

/// A cross-id / link entry (`externalLinks`). Only links that carry a numeric
/// `id` are identity-bearing; url-only links are not external ids.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExternalLink {
    pub site: Option<String>,
    pub id: Option<i64>,
    pub url: Option<String>,
}
