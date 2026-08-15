//! Google Books response models (MISSION-058).
//!
//! Google Books is **camelCase** end-to-end (`volumeInfo`, `industryIdentifiers`,
//! `publishedDate`, `imageLinks`), so nested structures use `rename_all =
//! "camelCase"`. Only the fields the adapter reads are declared.

use serde::Deserialize;

/// `/volumes?q=...` — search results.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VolumesResponse {
    #[serde(default)]
    pub items: Vec<Volume>,
}

/// A volume — used for both search rows and `/volumes/{id}` details.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Volume {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub volume_info: Option<VolumeInfo>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VolumeInfo {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default)]
    pub authors: Option<Vec<String>>,
    /// Free-form: `"1965"`, `"1965-08"`, `"1965-08-01"`, `"0000-00-00"` (unknown).
    #[serde(default)]
    pub published_date: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub page_count: Option<i64>,
    #[serde(default)]
    pub categories: Option<Vec<String>>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub image_links: Option<ImageLinks>,
    #[serde(default)]
    pub industry_identifiers: Option<Vec<IndustryIdentifier>>,
    #[serde(default)]
    pub canonical_volume_link: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImageLinks {
    #[serde(default)]
    pub thumbnail: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IndustryIdentifier {
    /// `"ISBN_10"`, `"ISBN_13"`, `"ISSN"`, `"OTHER"`.
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub identifier: Option<String>,
}
