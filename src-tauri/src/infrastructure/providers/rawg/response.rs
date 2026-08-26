//! RAWG API response models (MISSION-109).

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct SearchResponse {
    #[serde(default)]
    pub results: Vec<SearchItem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SearchItem {
    pub id: i64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub released: Option<String>,
    #[serde(default, rename = "background_image")]
    pub background_image: Option<String>,
    #[serde(default)]
    pub rating: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GameDetail {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(rename = "name_original", default)]
    pub name_original: Option<String>,
    #[serde(default, rename = "description_raw")]
    pub description_raw: Option<String>,
    #[serde(default)]
    pub released: Option<String>,
    #[serde(default, rename = "background_image")]
    pub background_image: Option<String>,
    #[serde(default)]
    pub metacritic: Option<i64>,
    #[serde(default)]
    pub genres: Vec<GenreRow>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GenreRow {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub name: Option<String>,
}
