//! Hardcover GraphQL response models (MISSION-064).
//!
//! Hardcover is a Hasura GraphQL service (`api.hardcover.app/v1/graphql`) —
//! snake_case fields unless noted. The `search` root returns Typesense JSON
//! blobs, so `results` is parsed defensively as `serde_json::Value`; the typed
//! `books` root mirrors the Books schema reference. Only read fields declared.

use serde::Deserialize;
use serde_json::Value;

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

/// `data.search` — Typesense-backed search results. `results` is a JSON blob
/// array (the docs' "huge blob of data" to parse yourself).
#[derive(Debug, Deserialize)]
pub(crate) struct SearchPayload {
    #[serde(rename = "search")]
    pub search: SearchResults,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SearchResults {
    #[serde(default)]
    pub results: Vec<Value>,
}

/// One search-result row parsed out of the Typesense blob. Typesense may
/// return ids as strings or numbers, so `id` goes through `flexible_id`. Only
/// fields the candidate mapping reads are declared (the blob carries far more).
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SearchBook {
    #[serde(
        default,
        deserialize_with = "crate::infrastructure::providers::hardcover::response::flexible_id"
    )]
    pub id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub release_year: Option<i64>,
}

/// Accept a Typesense id whether the API sends it as a string or a number.
pub(crate) fn flexible_id<'de, D>(de: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value: Option<Value> = Option::deserialize(de)?;
    Ok(value.and_then(|v| match v {
        Value::Number(n) => Some(n.to_string()),
        Value::String(s) if !s.is_empty() => Some(s),
        _ => None,
    }))
}

/// `data.books` — `books(where: …)` detail results.
#[derive(Debug, Deserialize)]
pub(crate) struct BooksData {
    #[serde(default)]
    pub books: Vec<Book>,
}

/// The full Book shape requested by DETAILS_QUERY. Hasura returns GraphQL field
/// names verbatim, so the struct mirrors the underscore query fields exactly
/// (`release_date`, `release_year`, `book_category_id`, `cached_tags`).
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Book {
    pub id: i64,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub release_date: Option<String>,
    #[serde(default)]
    pub release_year: Option<i64>,
    #[serde(default)]
    pub pages: Option<i64>,
    #[serde(default)]
    pub book_category_id: Option<i64>,
    #[serde(default)]
    pub cached_tags: Option<Value>,
    #[serde(default)]
    pub image: Option<Image>,
    #[serde(default)]
    pub contributions: Option<Vec<Contribution>>,
    #[serde(default)]
    pub editions: Option<Vec<Edition>>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Image {
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Contribution {
    #[serde(default)]
    pub author: Option<Author>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Author {
    #[serde(default)]
    pub name: Option<String>,
}

/// Editions carry the ISBNs. Their GraphQL fields keep the **underscore** names
/// (`isbn_10`, `isbn_13`) — no camelCase rename here.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Edition {
    #[serde(rename = "isbn_10", default)]
    pub isbn_10: Option<String>,
    #[serde(rename = "isbn_13", default)]
    pub isbn_13: Option<String>,
}
