//! Jikan (unofficial MyAnimeList) response models (MISSION-058).
//!
//! Jikan v4 is mostly snake_case (`mal_id`, `title_english`, `image_url`) with
//! camelCase only for the pagination wrapper (`last_visible_page`). Fields we
//! don't read are dropped; every field is `Option` because Jikan omits nulls.

use serde::Deserialize;

/// `/anime?q=...` — search results.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct AnimeSearchResponse {
    #[serde(default)]
    pub data: Vec<Anime>,
}

/// `/anime/{id}` — full details.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct AnimeDetailResponse {
    #[serde(default)]
    pub data: Option<Anime>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Anime {
    /// The MyAnimeList id — this is the Jikan provider id.
    pub mal_id: Option<i64>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub images: Option<Images>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub title_japanese: Option<String>,
    /// e.g. `"TV"`, `"Movie"`, `"OVA"`, `"ONA"`, `"Special"`, `"Music"`.
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub episodes: Option<i64>,
    /// e.g. `"Finished Airing"`, `"Currently Airing"`, `"Not yet aired"`.
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub year: Option<i64>,
    #[serde(default)]
    pub synopsis: Option<String>,
    #[serde(default)]
    pub aired: Option<Aired>,
    #[serde(default)]
    pub duration: Option<String>,
    #[serde(default)]
    pub rating: Option<String>,
    #[serde(default)]
    pub genres: Option<Vec<Genre>>,
    #[serde(default)]
    pub themes: Option<Vec<Genre>>,
    #[serde(default)]
    pub studios: Option<Vec<Studio>>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Images {
    #[serde(default)]
    pub jpg: Option<JpgImages>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct JpgImages {
    #[serde(default)]
    pub image_url: Option<String>,
    #[serde(default)]
    pub large_image_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Aired {
    /// ISO-8601 with timezone, e.g. `"2009-04-05T00:00:00+00:00"`.
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Genre {
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Studio {
    #[serde(default)]
    pub name: Option<String>,
}

/// `/anime/{id}/episodes` — the episode list (paginated, 100/page).
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct EpisodesResponse {
    #[serde(default)]
    pub data: Vec<Episode>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Episode {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub title_romanji: Option<String>,
}
