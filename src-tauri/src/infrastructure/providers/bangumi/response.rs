//! Bangumi v0 response models (MISSION-066, API_PROVIDERS §13).
//!
//! Bangumi's v0 API is OpenAPI 3.0 JSON at `api.bgm.tv/v0`. Camel-case field
//! names throughout (the opposite gotcha from the REST/GraphQL adapters). The
//! `Paged_Subject`/`Paged_Episode` shapes wrap `data`; the detail `Subject`
//! carries a `WikiV0` `infobox` whose values are either a string or a list of
//! `{ "v": … }` objects. Only read fields are declared.

use serde::Deserialize;

/// `{ total, limit, offset, data: [Subject] }` — the search response. Search
/// rows are the slim shape: no `summary`/`infobox`, but `short_summary` and the
/// `platform` sub-type (kept for book → manga/novel detection).
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PagedSubject {
    #[serde(default)]
    pub data: Vec<SlimSubject>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SlimSubject {
    pub id: i64,
    #[serde(default)]
    pub r#type: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub name_cn: String,
    #[serde(default)]
    pub short_summary: Option<String>,
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default)]
    pub images: Option<Images>,
}

/// `{ total, limit, offset, data: [Episode] }` from `GET /v0/episodes`.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PagedEpisode {
    #[serde(default)]
    pub data: Vec<Episode>,
}

/// A full `GET /v0/subjects/{id}` subject. `collection` is read-only data we
/// don't surface yet, so only the fields the normalizer reads are declared.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Subject {
    pub id: i64,
    #[serde(default)]
    pub r#type: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub name_cn: String,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default)]
    pub images: Option<Images>,
    #[serde(default)]
    pub infobox: Vec<InfoboxItem>,
    #[serde(default)]
    pub total_episodes: Option<i64>,
    #[serde(default)]
    pub tags: Vec<Tag>,
}

/// A `WikiV0` infobox entry. The value is either a plain string or an array of
/// `{ "v": … }` objects (multi-value fields like `别名`/`作者`).
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct InfoboxItem {
    pub key: String,
    #[serde(default)]
    pub value: Option<InfoboxValue>,
}

/// An infobox value: text, or a list of `{ "v" }` entries. Serialized
/// `untagged` so either JSON form deserializes.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(crate) enum InfoboxValue {
    Text(String),
    List(Vec<InfoboxEntry>),
}

impl InfoboxValue {
    /// The value flattened to one string (`None` when blank). Lists join with
    /// `、` (Bangumi's own separator for multi-value wiki fields).
    pub fn text(&self) -> Option<String> {
        match self {
            Self::Text(s) => clean_opt(s),
            Self::List(entries) => {
                let parts: Vec<String> = entries.iter().filter_map(|e| clean_opt(&e.v)).collect();
                if parts.is_empty() {
                    None
                } else {
                    Some(parts.join("、"))
                }
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct InfoboxEntry {
    #[serde(default)]
    pub v: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Images {
    #[serde(default)]
    pub large: Option<String>,
}

/// A user/wiki tag with vote counts.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Tag {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub count: i64,
}

/// A chapter/episode row from `GET /v0/episodes`. `ep` is the number within the
/// subject (1-based for main-story rows); `sort` is the all-seasons sort and is
/// what specials without an `ep` carry. Both can be fractional (`1.5`).
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Episode {
    pub id: i64,
    #[serde(default)]
    pub r#type: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub name_cn: String,
    #[serde(default)]
    pub ep: Option<f64>,
    #[serde(default)]
    pub sort: Option<f64>,
    #[serde(default)]
    pub airdate: Option<String>,
    #[serde(default)]
    pub duration_seconds: Option<i64>,
    #[serde(default)]
    pub desc: Option<String>,
}

/// One edge from `GET /v0/subjects/{id}/subjects`. `relation` is a Chinese wiki
/// label (`前传`, `续集`, `原作`, `动画`, …).
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RelatedSubject {
    pub id: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub name_cn: String,
    #[serde(default)]
    pub relation: Option<String>,
}

fn clean_opt(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}
