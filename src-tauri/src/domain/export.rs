//! Export model + formatters (MISSION-071, ARCHITECTURE §6 / REQ-EXPORT-001).
//!
//! Pure, DB-free pieces of the export feature: the `ExportFormat` enum, the
//! flattened `ExportRow` one row maps to, and the three renderers (JSON value,
//! CSV header + fields, human-readable Markdown section). The streaming write
//! lives in `application::export_service`; this module never touches a pool.
//!
//! The JSON output is deliberately shaped like the MISSION-068 import format
//! (same field names, `people`/`external_ids` object arrays, multi-value
//! arrays), so an export can be re-imported unchanged. User data — tracking
//! status, rating/review/notes, favorite, collections — rides along as extra
//! keys the import parser ignores.

use std::fmt;
use std::str::FromStr;

use serde::Serialize;

use crate::error::AppError;

/// Which serialization an export uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Json,
    Csv,
    Markdown,
}

impl ExportFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Csv => "csv",
            Self::Markdown => "markdown",
        }
    }

    /// File extension for the save dialog + default filename.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Csv => "csv",
            Self::Markdown => "md",
        }
    }
}

impl fmt::Display for ExportFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ExportFormat {
    type Err = AppError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        match raw.to_ascii_lowercase().as_str() {
            "json" => Ok(Self::Json),
            "csv" => Ok(Self::Csv),
            "markdown" | "md" => Ok(Self::Markdown),
            other => Err(AppError::validation(format!(
                "unknown export format: {other} (expected json, csv or markdown)"
            ))),
        }
    }
}

/// A person credit on an exported row (`role` + resolved name).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExportPerson {
    pub role: String,
    pub name: String,
}

/// An external identity (`provider:value`, with an optional url).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExportExternalId {
    pub provider: String,
    pub value: String,
    pub url: Option<String>,
}

/// One fully-flattened library record. Field names match the import format so
/// a JSON export round-trips through MISSION-068; the trailing user-data fields
/// are ignored by the import parser.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExportRow {
    pub title: String,
    pub title_original: Option<String>,
    pub alt_titles: Vec<String>,
    pub content_type: String,
    pub format: Option<String>,
    pub pub_status: String,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub release_year: Option<i64>,
    pub language: Option<String>,
    pub country: Option<String>,
    pub content_rating: Option<String>,
    pub pages: Option<i64>,
    pub duration_min: Option<i64>,
    pub ep_count: Option<i64>,
    pub ch_count: Option<i64>,
    pub synopsis: Option<String>,
    pub people: Vec<ExportPerson>,
    pub genres: Vec<String>,
    pub tags: Vec<String>,
    pub external_ids: Vec<ExportExternalId>,
    pub cover_url: Option<String>,
    pub banner_url: Option<String>,
    // User data (extra keys beyond the import format).
    pub my_status: Option<String>,
    pub my_rating: Option<i64>,
    pub my_review: Option<String>,
    pub my_short_review: Option<String>,
    pub my_notes: Option<String>,
    pub favorite: bool,
    pub collections: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// JSON: the row as a `serde_json::Value` whose keys already match the import
/// format, so the whole export is a plain array of these objects.
pub fn row_to_json(row: &ExportRow) -> Result<serde_json::Value, serde_json::Error> {
    serde_json::to_value(row)
}

/// CSV: the fixed column order for every export (import-mapping fields first,
/// then the user-data columns).
pub const CSV_HEADERS: &[&str] = &[
    "title",
    "title_original",
    "alt_titles",
    "content_type",
    "format",
    "pub_status",
    "start_date",
    "end_date",
    "release_year",
    "language",
    "country",
    "content_rating",
    "pages",
    "duration_min",
    "ep_count",
    "ch_count",
    "synopsis",
    "author",
    "artist",
    "studio",
    "genres",
    "tags",
    "external_id",
    "cover_url",
    "banner_url",
    "my_status",
    "my_rating",
    "my_review",
    "my_short_review",
    "my_notes",
    "favorite",
    "collections",
    "created_at",
    "updated_at",
];

/// Multi-value cells (alt titles, genres, tags, collections, people per role,
/// external ids) join on this list separator, which never collides with the
/// `,` field delimiter.
pub const LIST_SEPARATOR: &str = "|";

/// CSV: one record as raw strings, in `CSV_HEADERS` order. Nulls become empty
/// strings; multi-value fields join on `|`; external ids become
/// `provider:value` cells so the MISSION-068 CSV import can re-read them.
pub fn row_to_csv(row: &ExportRow) -> Vec<String> {
    fn join(values: &[String]) -> String {
        values.join(LIST_SEPARATOR)
    }

    let by_role = |role: &str| -> Vec<String> {
        row.people
            .iter()
            .filter(|person| person.role == role)
            .map(|person| person.name.clone())
            .collect()
    };

    vec![
        row.title.clone(),
        row.title_original.clone().unwrap_or_default(),
        join(&row.alt_titles),
        row.content_type.clone(),
        row.format.clone().unwrap_or_default(),
        row.pub_status.clone(),
        row.start_date.clone().unwrap_or_default(),
        row.end_date.clone().unwrap_or_default(),
        opt_str(row.release_year),
        row.language.clone().unwrap_or_default(),
        row.country.clone().unwrap_or_default(),
        row.content_rating.clone().unwrap_or_default(),
        opt_str(row.pages),
        opt_str(row.duration_min),
        opt_str(row.ep_count),
        opt_str(row.ch_count),
        row.synopsis.clone().unwrap_or_default(),
        join(&by_role("author")),
        join(&by_role("artist")),
        join(&by_role("studio")),
        join(&row.genres),
        join(&row.tags),
        join(
            &row.external_ids
                .iter()
                .map(|id| format!("{}:{}", id.provider, id.value))
                .collect::<Vec<_>>(),
        ),
        row.cover_url.clone().unwrap_or_default(),
        row.banner_url.clone().unwrap_or_default(),
        row.my_status.clone().unwrap_or_default(),
        opt_str(row.my_rating),
        row.my_review.clone().unwrap_or_default(),
        row.my_short_review.clone().unwrap_or_default(),
        row.my_notes.clone().unwrap_or_default(),
        if row.favorite { "true" } else { "false" }.to_string(),
        join(&row.collections),
        row.created_at.clone(),
        row.updated_at.clone(),
    ]
}

fn opt_str(value: Option<i64>) -> String {
    value.map(|v| v.to_string()).unwrap_or_default()
}

/// Markdown: a human-readable section for one title.
pub fn render_markdown(row: &ExportRow) -> String {
    let mut out = String::new();

    let title = match &row.title_original {
        Some(original) => format!("{} ({})", row.title, original),
        None => row.title.clone(),
    };
    out.push_str(&format!("# {title}\n"));

    if !row.alt_titles.is_empty() {
        out.push_str(&format!(
            "**Also known as:** {}\n",
            row.alt_titles.join(", ")
        ));
    }

    let mut facts: Vec<String> = Vec::new();
    facts.push(row.content_type.clone());
    if let Some(format) = &row.format {
        facts.push(format.clone());
    }
    facts.push(row.pub_status.clone());
    if let Some(year) = row.release_year {
        facts.push(year.to_string());
    }
    if !facts.is_empty() {
        out.push_str(&format!("**{}**\n", facts.join(" · ")));
    }

    if let Some(rating) = &row.content_rating {
        out.push_str(&format!("**Content rating:** {rating}\n"));
    }

    let mut meta: Vec<String> = Vec::new();
    if let Some(language) = &row.language {
        meta.push(format!("Language: {language}"));
    }
    if let Some(country) = &row.country {
        meta.push(format!("Country: {country}"));
    }
    if let Some(pages) = row.pages {
        meta.push(format!("Pages: {pages}"));
    }
    if let Some(duration) = row.duration_min {
        meta.push(format!("Duration: {duration} min"));
    }
    if let Some(episodes) = row.ep_count {
        meta.push(format!("Episodes: {episodes}"));
    }
    if let Some(chapters) = row.ch_count {
        meta.push(format!("Chapters: {chapters}"));
    }
    if let Some(start) = &row.start_date {
        let mut range = start.clone();
        if let Some(end) = &row.end_date {
            range.push_str(&format!(" → {end}"));
        }
        meta.push(format!("Dates: {range}"));
    }
    if !meta.is_empty() {
        out.push_str(&format!("**{}**\n", meta.join(" · ")));
    }

    for person in &row.people {
        out.push_str(&format!(
            "**{}:** {}\n",
            capitalize(&person.role),
            person.name
        ));
    }

    if !row.genres.is_empty() {
        out.push_str(&format!("**Genres:** {}\n", row.genres.join(", ")));
    }
    if !row.tags.is_empty() {
        out.push_str(&format!("**Tags:** {}\n", row.tags.join(", ")));
    }
    if !row.external_ids.is_empty() {
        let ids = row
            .external_ids
            .iter()
            .map(|id| format!("{} ({})", id.provider, id.value))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("**External IDs:** {ids}\n"));
    }

    let mut mine: Vec<String> = Vec::new();
    if let Some(status) = &row.my_status {
        mine.push(format!("Status: {status}"));
    }
    if let Some(rating) = row.my_rating {
        mine.push(format!("My rating: {rating}/10"));
    }
    if row.favorite {
        mine.push("Favorite".to_string());
    }
    if !row.collections.is_empty() {
        mine.push(format!("Collections: {}", row.collections.join(", ")));
    }
    if !mine.is_empty() {
        out.push_str(&format!("**My data:** {}\n", mine.join(" · ")));
    }

    if let Some(review) = &row.my_review {
        if !review.trim().is_empty() {
            out.push_str(&format!("\n**Review:**\n{review}\n"));
        }
    }
    if let Some(notes) = &row.my_notes {
        if !notes.trim().is_empty() {
            out.push_str(&format!("\n**Notes:**\n{notes}\n"));
        }
    }
    if let Some(synopsis) = &row.synopsis {
        if !synopsis.trim().is_empty() {
            out.push_str(&format!("\n{synopsis}\n"));
        }
    }

    out
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_row() -> ExportRow {
        ExportRow {
            title: "Sword of the Dawn".to_string(),
            title_original: Some("夜明けの剣".to_string()),
            alt_titles: vec!["Dawn's Sword".to_string(), "Sword of Dawn".to_string()],
            content_type: "novel".to_string(),
            format: Some("light_novel".to_string()),
            pub_status: "ongoing".to_string(),
            start_date: Some("2025-01-01".to_string()),
            end_date: Some("2025-06-30".to_string()),
            release_year: Some(2025),
            language: Some("ja".to_string()),
            country: Some("JP".to_string()),
            content_rating: Some("Teen".to_string()),
            pages: Some(320),
            duration_min: None,
            ep_count: None,
            ch_count: Some(12),
            synopsis: Some("A tale.\nSecond line.".to_string()),
            people: vec![
                ExportPerson {
                    role: "author".to_string(),
                    name: "Jane".to_string(),
                },
                ExportPerson {
                    role: "artist".to_string(),
                    name: "Mira".to_string(),
                },
            ],
            genres: vec!["Fantasy".to_string(), "Adventure".to_string()],
            tags: vec!["isekai".to_string()],
            external_ids: vec![ExportExternalId {
                provider: "anilist".to_string(),
                value: "42".to_string(),
                url: Some("https://anilist.co/anime/42".to_string()),
            }],
            cover_url: Some("https://cdn.example/cover.jpg".to_string()),
            banner_url: None,
            my_status: Some("reading".to_string()),
            my_rating: Some(8),
            my_review: Some("Lovely.".to_string()),
            my_short_review: None,
            my_notes: Some("read with tea".to_string()),
            favorite: true,
            collections: vec!["Favorites shelf".to_string()],
            created_at: "2026-08-16T00:00:00Z".to_string(),
            updated_at: "2026-08-16T01:00:00Z".to_string(),
        }
    }

    #[test]
    fn format_parses_and_displays() {
        assert_eq!("json".parse::<ExportFormat>().unwrap(), ExportFormat::Json);
        assert_eq!("CSV".parse::<ExportFormat>().unwrap(), ExportFormat::Csv);
        assert_eq!(
            "markdown".parse::<ExportFormat>().unwrap(),
            ExportFormat::Markdown
        );
        assert_eq!(
            "md".parse::<ExportFormat>().unwrap(),
            ExportFormat::Markdown
        );
        assert_eq!(ExportFormat::Json.as_str(), "json");
        assert_eq!(ExportFormat::Json.to_string(), "json");
        assert_eq!(ExportFormat::Json.extension(), "json");
        assert_eq!(ExportFormat::Csv.extension(), "csv");
        assert_eq!(ExportFormat::Markdown.extension(), "md");
        assert!("xml".parse::<ExportFormat>().is_err());
    }

    #[test]
    fn json_row_uses_import_field_names() {
        let value = row_to_json(&sample_row()).unwrap();
        assert_eq!(value["title"], "Sword of the Dawn");
        assert_eq!(value["content_type"], "novel");
        assert_eq!(value["pages"], 320);
        assert_eq!(value["people"][0]["role"], "author");
        assert_eq!(value["people"][0]["name"], "Jane");
        assert_eq!(value["external_ids"][0]["provider"], "anilist");
        assert_eq!(
            value["alt_titles"],
            json!(["Dawn's Sword", "Sword of Dawn"])
        );
        assert_eq!(value["my_status"], "reading");
        assert_eq!(value["favorite"], true);
        assert_eq!(value["collections"], json!(["Favorites shelf"]));
        assert!(value["duration_min"].is_null());
    }

    #[test]
    fn csv_headers_are_fixed_and_row_aligns() {
        let fields = row_to_csv(&sample_row());
        assert_eq!(fields.len(), CSV_HEADERS.len());
        let field = |name: &str| {
            let index = CSV_HEADERS.iter().position(|h| *h == name).unwrap();
            fields[index].clone()
        };
        assert_eq!(field("title"), "Sword of the Dawn");
        assert_eq!(field("alt_titles"), "Dawn's Sword|Sword of Dawn");
        assert_eq!(field("author"), "Jane");
        assert_eq!(field("artist"), "Mira");
        assert_eq!(field("genres"), "Fantasy|Adventure");
        assert_eq!(field("tags"), "isekai");
        assert_eq!(field("external_id"), "anilist:42");
        assert_eq!(field("my_rating"), "8");
        assert_eq!(field("favorite"), "true");
        assert_eq!(field("collections"), "Favorites shelf");
        assert_eq!(field("duration_min"), "");
        assert_eq!(field("banner_url"), "");
    }

    #[test]
    fn csv_row_without_user_data_uses_empty_strings() {
        let mut row = sample_row();
        row.my_status = None;
        row.my_rating = None;
        row.favorite = false;
        row.collections.clear();
        row.people.clear();
        row.alt_titles.clear();
        let fields = row_to_csv(&row);
        let field = |name: &str| {
            let index = CSV_HEADERS.iter().position(|h| *h == name).unwrap();
            fields[index].clone()
        };
        assert_eq!(field("my_status"), "");
        assert_eq!(field("my_rating"), "");
        assert_eq!(field("favorite"), "false");
        assert_eq!(field("collections"), "");
        assert_eq!(field("author"), "");
        assert_eq!(field("alt_titles"), "");
    }

    #[test]
    fn markdown_renders_human_readable_section() {
        let md = render_markdown(&sample_row());
        assert!(md.starts_with("# Sword of the Dawn (夜明けの剣)\n"));
        assert!(md.contains("**Also known as:** Dawn's Sword, Sword of Dawn"));
        assert!(md.contains("**novel · light_novel · ongoing · 2025**"));
        assert!(md.contains("**Language: ja · Country: JP · Pages: 320 · Chapters: 12 · Dates: 2025-01-01 → 2025-06-30**"));
        assert!(md.contains("**Author:** Jane"));
        assert!(md.contains("**Artist:** Mira"));
        assert!(md.contains("**Genres:** Fantasy, Adventure"));
        assert!(md.contains("**External IDs:** anilist (42)"));
        assert!(md.contains("**My data:** Status: reading · My rating: 8/10 · Favorite · Collections: Favorites shelf"));
        assert!(md.contains("\n**Review:**\nLovely.\n"));
        assert!(md.contains("\nA tale.\nSecond line.\n"));
    }

    #[test]
    fn markdown_skips_empty_optional_sections() {
        let row = ExportRow {
            title: "Berserk".to_string(),
            title_original: None,
            alt_titles: vec![],
            content_type: "manga".to_string(),
            format: None,
            pub_status: "completed".to_string(),
            start_date: None,
            end_date: None,
            release_year: None,
            language: None,
            country: None,
            content_rating: None,
            pages: None,
            duration_min: None,
            ep_count: None,
            ch_count: None,
            synopsis: None,
            people: vec![],
            genres: vec![],
            tags: vec![],
            external_ids: vec![],
            cover_url: None,
            banner_url: None,
            my_status: None,
            my_rating: None,
            my_review: None,
            my_short_review: None,
            my_notes: None,
            favorite: false,
            collections: vec![],
            created_at: "2026-08-16T00:00:00Z".to_string(),
            updated_at: "2026-08-16T00:00:00Z".to_string(),
        };
        let md = render_markdown(&row);
        assert!(md.starts_with("# Berserk\n"));
        assert!(md.contains("**manga · completed**"));
        assert!(!md.contains("**Also known as:**"));
        assert!(!md.contains("**Author:**"));
        assert!(!md.contains("**My data:**"));
        assert!(!md.contains("**Review:**"));
        assert!(!md.contains("**Notes:**"));
    }
}
