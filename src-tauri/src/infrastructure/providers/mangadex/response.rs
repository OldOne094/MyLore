//! Serde models for MangaDex REST responses (MISSION-056).
//!
//! MangaDex v5 uses camelCase for multi-word attributes (`altTitles`,
//! `originalLanguage`, `publishAt`, `fileName`) but single words for the rest,
//! so attributes structs use `rename_all = "camelCase"` while top-level
//! envelope fields (`data`, `result`, `type`, `id`) keep their default names.
//! Only fields the adapter reads are declared — serde ignores the rest.

use std::collections::HashMap;

use serde::Deserialize;

/// `GET /manga` (list/search).
#[derive(Debug, Deserialize)]
pub(crate) struct MangaListResponse {
    pub data: Vec<Manga>,
}

/// `GET /manga/{id}`.
#[derive(Debug, Deserialize)]
pub(crate) struct MangaSingleResponse {
    pub data: Option<Manga>,
}

/// `GET /manga/{id}/feed` (chapter list, ordered by volume then chapter).
#[derive(Debug, Deserialize)]
pub(crate) struct FeedResponse {
    pub data: Vec<Chapter>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Manga {
    pub id: String,
    pub attributes: MangaAttributes,
    pub relationships: Vec<Relationship>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MangaAttributes {
    /// locale → title (en, ja, ko, zh, ar…).
    pub title: HashMap<String, String>,
    /// each entry is a `{ locale: title }` map.
    pub alt_titles: Vec<HashMap<String, String>>,
    /// locale → description.
    pub description: HashMap<String, String>,
    pub original_language: Option<String>,
    pub year: Option<i64>,
    pub status: Option<String>,
    pub format: Option<String>,
    pub tags: Vec<Tag>,
    pub content_rating: Option<String>,
    /// External ids: `{ mal, anilist, bw, nu, kt, ap, mu, … }`.
    pub links: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Tag {
    pub attributes: TagAttributes,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TagAttributes {
    /// locale → localized tag name.
    pub name: HashMap<String, String>,
    /// `genre` / `theme` / `demographic` / `format` / `content`.
    pub group: Option<String>,
}

/// A relationship edge. `kind` is `author`/`artist`/`cover_art`/…; the
/// included `attributes` carry the author/artist `name` or the cover
/// `fileName`.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Relationship {
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub attributes: Option<RelationshipAttributes>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RelationshipAttributes {
    pub name: Option<String>,
    pub file_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Chapter {
    pub id: String,
    pub attributes: ChapterAttributes,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChapterAttributes {
    pub volume: Option<String>,
    pub chapter: Option<String>,
    pub title: Option<String>,
    pub publish_at: Option<String>,
    pub pages: Option<i64>,
}
