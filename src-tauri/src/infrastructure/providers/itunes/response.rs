//! iTunes Search response models (MISSION-109).

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct SearchResponse {
    #[serde(default)]
    pub results: Vec<SearchItem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SearchItem {
    /// Unique numeric id (`collectionId`).
    pub collection_id: Option<i64>,
    #[serde(rename = "collectionName")]
    pub collection_name: Option<String>,
    #[serde(rename = "artistName")]
    pub artist_name: Option<String>,
    #[serde(rename = "artworkUrl100")]
    pub artwork_url_100: Option<String>,
    #[serde(rename = "artworkUrl600")]
    pub artwork_url_600: Option<String>,
    #[serde(rename = "releaseDate")]
    pub release_date: Option<String>,
    #[serde(rename = "trackCount")]
    pub track_count: Option<i64>,
    #[serde(rename = "primaryGenreName")]
    pub primary_genre_name: Option<String>,
    #[serde(default)]
    pub genres: Vec<String>,
    #[serde(rename = "collectionViewUrl")]
    pub collection_view_url: Option<String>,
    #[serde(rename = "contentAdvisoryRating")]
    pub content_advisory_rating: Option<String>,
}
