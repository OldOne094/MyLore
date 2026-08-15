//! Serde models for TMDB REST responses (MISSION-055).
//!
//! These mirror the exact TMDB shapes (`snake_case` throughout) so
//! normalization is a pure mapping â€” Rust field names already match the JSON,
//! so no `rename_all` is needed (unlike AniList's GraphQL camelCase). Only
//! fields the adapter reads are declared â€” serde ignores the rest (keeps
//! fixtures honest without dead-code warnings). `MediaDetails` is shared by
//! `/movie/{id}` and `/tv/{id}`; per-kind fields (`title` vs `name`,
//! `release_date` vs `first_air_date`, `runtime` vs `episode_run_time`) are
//! `Option` and the adapter knows the kind from its provider id prefix
//! (`movie-<id>` / `tv-<id>`).

use serde::Deserialize;

/// `GET /search/multi`.
#[derive(Debug, Deserialize)]
pub(crate) struct SearchResponse {
    pub results: Vec<SearchResult>,
}

/// One row of `/search/multi`. `media_type` is `movie`/`tv`/`person`/â€¦ â€” we
/// only keep `movie` and `tv` rows.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SearchResult {
    #[serde(rename = "media_type")]
    pub media_type: Option<String>,
    pub id: i64,
    pub title: Option<String>,
    pub name: Option<String>,
    pub original_title: Option<String>,
    pub original_name: Option<String>,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub release_date: Option<String>,
    pub first_air_date: Option<String>,
}

/// Shared shape for `/movie/{id}` and `/tv/{id}` (with
/// `append_to_response=credits,external_ids`).
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct MediaDetails {
    pub id: i64,
    pub title: Option<String>,
    pub name: Option<String>,
    pub original_title: Option<String>,
    pub original_name: Option<String>,
    pub overview: Option<String>,
    pub release_date: Option<String>,
    pub first_air_date: Option<String>,
    pub last_air_date: Option<String>,
    pub runtime: Option<i64>,
    pub episode_run_time: Option<Vec<i64>>,
    pub number_of_episodes: Option<i64>,
    pub status: Option<String>,
    pub genres: Option<Vec<Genre>>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub homepage: Option<String>,
    pub original_language: Option<String>,
    pub production_countries: Option<Vec<Country>>,
    pub production_companies: Option<Vec<Company>>,
    pub networks: Option<Vec<Company>>,
    pub seasons: Option<Vec<SeasonInfo>>,
    pub credits: Option<Credits>,
    pub external_ids: Option<ExternalIds>,
}

/// One season listed in a TV show's details. Episode lists are fetched per
/// season (`/tv/{id}/season/{n}`); only the season numbers matter here.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SeasonInfo {
    pub season_number: Option<i64>,
}

/// `GET /tv/{id}/season/{n}` â€” the episode list that becomes the node tree.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SeasonDetails {
    pub season_number: Option<i64>,
    pub name: Option<String>,
    pub air_date: Option<String>,
    pub overview: Option<String>,
    pub episodes: Vec<Episode>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Episode {
    pub episode_number: Option<i64>,
    pub name: Option<String>,
    pub overview: Option<String>,
    pub air_date: Option<String>,
    pub runtime: Option<i64>,
}

/// `GET /movie/{id}/external_ids` / `GET /tv/{id}/external_ids`.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ExternalIds {
    pub imdb_id: Option<String>,
    pub tvdb_id: Option<i64>,
    pub tvrage_id: Option<i64>,
    pub wikidata_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Genre {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Country {
    #[serde(rename = "iso_3166_1")]
    pub iso_3166_1: Option<String>,
}

/// Production companies (â†’ Studio) and networks (â†’ Network) share this shape.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Company {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Credits {
    pub crew: Vec<CrewMember>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CrewMember {
    pub name: Option<String>,
    pub job: Option<String>,
}
