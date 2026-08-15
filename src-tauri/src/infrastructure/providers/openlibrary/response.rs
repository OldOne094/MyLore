//! OpenLibrary API response models (MISSION-057).
//!
//! OpenLibrary is snake_case end-to-end (unlike AniList/TMDB/MangaDex, which
//! each needed a rename), so these models are plain Rust field names. Only the
//! fields the adapter reads are declared; everything else is dropped.

use serde::Deserialize;

/// `/search.json?q=...` — the generic catalog search. Returns an array of
/// flattened `docs` (title, authors, first year, cover id, subjects, ISBNs).
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SearchResponse {
    #[serde(default)]
    pub docs: Vec<SearchDoc>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SearchDoc {
    /// e.g. `/works/OL89650W`
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub first_publish_year: Option<i64>,
    /// The `covers.id` used by `covers.openlibrary.org` (may be absent).
    #[serde(default)]
    pub cover_i: Option<i64>,
}

/// `/works/{id}.json` — the canonical work record.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct WorkResponse {
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    /// Either a plain string or `{ "value": "...", "type": "/type/text" }`.
    #[serde(default)]
    pub description: Option<Description>,
    /// Free-form, e.g. `"1965"` or `"May 1989"` — never trust a full date.
    #[serde(default)]
    pub first_publish_date: Option<String>,
    #[serde(default)]
    pub authors: Option<Vec<WorkAuthor>>,
    #[serde(default)]
    pub subjects: Option<Vec<String>>,
    #[serde(default)]
    pub covers: Option<Vec<i64>>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct WorkAuthor {
    #[serde(default)]
    pub author: Option<AuthorRef>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct AuthorRef {
    #[serde(default)]
    pub key: Option<String>,
}

/// OpenLibrary `description` may be a string or a `{ value, type }` object.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(crate) enum Description {
    Text(String),
    Value(DescriptionValue),
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct DescriptionValue {
    #[serde(default)]
    pub value: Option<String>,
}

/// `/authors/{id}.json` — resolve author keys to display names.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct AuthorResponse {
    #[serde(default)]
    pub name: Option<String>,
}

/// `/works/{id}/editions.json?limit=1&fields=...` — the first edition, which
/// carries the ISBNs/LCCN/OCLC numbers used for dedup (API_PROVIDERS §7).
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct EditionsResponse {
    #[serde(default)]
    pub docs: Vec<Edition>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Edition {
    #[serde(default)]
    pub isbn_10: Option<Vec<String>>,
    #[serde(default)]
    pub isbn_13: Option<Vec<String>>,
    #[serde(default)]
    pub lccn: Option<Vec<String>>,
    #[serde(default)]
    pub oclc_numbers: Option<Vec<String>>,
}
