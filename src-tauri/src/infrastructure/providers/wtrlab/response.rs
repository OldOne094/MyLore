//! WTR-LAB response models + `__NEXT_DATA__` extraction (MISSION-131).
//!
//! Search/chapters are plain JSON APIs. Details live inside the novel page's
//! embedded `__NEXT_DATA__` script; only the `props.pageProps` subtree we
//! consume is modeled, everything else is skipped by serde.

use serde::Deserialize;

/// `{ success, data: [...] }` from `/api/search`.
#[derive(Debug, Deserialize)]
pub(crate) struct SearchResponse {
    #[serde(default)]
    pub data: Vec<SearchItem>,
}

/// One search row. `data` carries the display fields; `raw` mirrors them in
/// the source language.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SearchItem {
    pub id: i64,
    pub slug: String,

    #[serde(default)]
    pub data: ItemData,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub(crate) struct ItemData {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub raw: RawData,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub(crate) struct RawData {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
}

/// `{ chapters: [...] }` from `/api/chapters/{id}`.
#[derive(Debug, Deserialize)]
pub(crate) struct ChaptersResponse {
    #[serde(default)]
    pub chapters: Vec<ChapterRow>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ChapterRow {
    #[serde(default)]
    pub order: Option<i64>,
    #[serde(default)]
    pub title: Option<String>,
    /// Source-language title.
    #[serde(default)]
    pub name: Option<String>,
}

/// Parsed `__NEXT_DATA__` details payload (MISSION-131). Modeled after the
/// Python lab's extractor: `props.pageProps.serie.{serie_data,data,names}`
/// plus the structured `pageProps.tags`.
#[derive(Debug, Clone, Deserialize, Default)]
pub(crate) struct NextData {
    #[serde(default, rename = "props")]
    pub props: NextProps,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub(crate) struct NextProps {
    #[serde(default, rename = "pageProps")]
    pub page_props: PageProps,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub(crate) struct PageProps {
    #[serde(default)]
    pub serie: SerieRoot,
    #[serde(default)]
    pub tags: Vec<TagRow>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub(crate) struct SerieRoot {
    #[serde(default, rename = "serie_data")]
    pub serie_data: SerieData,
    #[serde(default)]
    pub names: Vec<NameRow>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub(crate) struct SerieData {
    /// Live structure: the display payload (title/author/description/image +
    /// source-language `raw`) nests *inside* `serie_data`, mirroring the
    /// shape of a `/api/search` row's `data`.
    #[serde(default)]
    pub data: ItemData,
    /// 1 = completed, 0 = ongoing.
    #[serde(default)]
    pub status: Option<i64>,
    #[serde(default, rename = "chapter_count")]
    pub chapter_count: Option<i64>,
    #[serde(default)]
    pub genres: Vec<i64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub(crate) struct NameRow {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default, rename = "raw_title")]
    pub raw_title: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub(crate) struct TagRow {
    #[serde(default)]
    pub title: Option<String>,
}

/// Pull the `__NEXT_DATA__` script body out of an HTML page.
pub(crate) fn extract_next_data(html: &str) -> Option<NextData> {
    const OPEN: &str = r#"<script id="__NEXT_DATA__" type="application/json">"#;
    let start = html.find(OPEN)? + OPEN.len();
    let end = html[start..].find("</script>")? + start;
    serde_json::from_str(html.get(start..end)?).ok()
}
