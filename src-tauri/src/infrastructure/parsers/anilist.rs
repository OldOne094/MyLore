//! The **AniList user export** JSON parser (MISSION-072, ARCHITECTURE §6).
//!
//! AniList lets a user export their whole list from Settings → Export; the file
//! is the result of their export GraphQL query:
//!
//! ```json
//! {
//!   "mediaListCollection": {
//!     "lists": [
//!       { "name": "Anime", "entries": [ { "media": { … }, "status": "COMPLETED",
//!         "score": 80, "progress": 12, "repeat": 0, "startedAt": {…}, "completedAt": {…},
//!         "notes": "…" } ] }
//!     ]
//!   }
//! }
//! ```
//!
//! Each entry's `media` object carries the metadata (titles, type/format,
//! status, dates, counts, covers, genres, tags, staff, description) and the
//! entry itself carries the user's list state (status → `my_status`, score →
//! `my_rating` on a 0–10 scale, progress, repeat, dates, notes → `my_review`).
//! Entries without a `media` object are skipped (no metadata to import).
//! A document that is not the AniList export shape aborts with
//! `ImportError::Parse`; per-entry problems are left to the pipeline's
//! validator/normalizer.

use serde_json::Value;

use crate::domain::import::{ImportError, ImportParser, ParsedItem};

/// Parses the AniList user export into pipeline items.
pub struct AniListParser;

impl ImportParser for AniListParser {
    fn parse(&self, source: &str) -> Result<Vec<ParsedItem>, ImportError> {
        let document: Value = serde_json::from_str(source)
            .map_err(|error| ImportError::Parse(format!("invalid JSON: {error}")))?;
        let lists = document
            .get("mediaListCollection")
            .and_then(|collection| collection.get("lists"))
            .and_then(Value::as_array)
            .ok_or_else(|| {
                ImportError::Parse(
                    "expected the AniList export shape (mediaListCollection.lists)".to_string(),
                )
            })?;

        let mut items = Vec::new();
        for list in lists {
            let Some(entries) = list.get("entries").and_then(Value::as_array) else {
                continue;
            };
            for entry in entries {
                let Some(media) = entry.get("media") else {
                    continue;
                };
                items.push(to_item(items.len() + 1, entry, media));
            }
        }
        Ok(items)
    }
}

fn to_item(row: usize, entry: &Value, media: &Value) -> ParsedItem {
    let title = pick_title(media.get("title"));
    let alt_titles = alt_titles(media.get("title"), &title);
    let title_original = original(media.get("title"), &title);
    let (content_type, format) = content_type_and_format(media);
    let pub_status = pub_status(media.get("status").and_then(Value::as_str));

    ParsedItem {
        row,
        title,
        title_original,
        alt_titles,
        content_type: Some(content_type.to_string()),
        format,
        pub_status,
        start_date: anilist_date(media.get("startDate")),
        end_date: anilist_date(media.get("endDate")),
        release_year: anilist_year(media.get("startDate")),
        language: None,
        country: None,
        content_rating: None,
        pages: None,
        duration_min: opt_number(media, "duration"),
        ep_count: opt_number(media, "episodes"),
        ch_count: opt_number(media, "chapters"),
        synopsis: opt_string(media, "description").map(strip_html),
        people: staff_people(media.get("staff")),
        genres: string_array(media.get("genres")),
        tags: tag_names(media.get("tags")),
        external_ids: external_ids(media),
        cover_url: cover_url(media.get("coverImage")),
        banner_url: opt_string(media, "bannerImage"),
        my_status: list_status(entry.get("status").and_then(Value::as_str)),
        my_rating: score_to_rating(entry.get("score")),
        my_review: opt_string(entry, "notes"),
        progress: opt_number(entry, "progress"),
        started_at: anilist_date(entry.get("startedAt")),
        completed_at: anilist_date(entry.get("completedAt")),
        repeat_count: opt_number(entry, "repeat"),
    }
}

fn pick_title(value: Option<&Value>) -> String {
    let Some(object) = value.and_then(Value::as_object) else {
        return String::new();
    };
    for key in ["userPreferred", "romaji", "english", "native"] {
        if let Some(title) = object.get(key).and_then(Value::as_str) {
            let title = title.trim();
            if !title.is_empty() {
                return title.to_string();
            }
        }
    }
    String::new()
}

/// The native title, kept only when it differs from the main one.
fn original(value: Option<&Value>, main: &str) -> Option<String> {
    let native = value
        .and_then(Value::as_object)
        .and_then(|object| object.get("native"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|title| !title.is_empty())?;
    if native.eq_ignore_ascii_case(main) {
        None
    } else {
        Some(native.to_string())
    }
}

/// English + romaji alternatives, deduplicated against the main title.
fn alt_titles(value: Option<&Value>, main: &str) -> Vec<String> {
    let Some(object) = value.and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for key in ["english", "romaji"] {
        if let Some(title) = object.get(key).and_then(Value::as_str) {
            let title = title.trim();
            if !title.is_empty()
                && !title.eq_ignore_ascii_case(main)
                && !out.iter().any(|t: &String| t.eq_ignore_ascii_case(title))
            {
                out.push(title.to_string());
            }
        }
    }
    out
}

/// `media.type` (ANIME | MANGA) with a format fallback for older exports.
fn content_type_and_format(media: &Value) -> (&'static str, Option<String>) {
    let format = opt_string(media, "format").map(|f| f.to_ascii_lowercase());
    let content_type = match media.get("type").and_then(Value::as_str) {
        Some("ANIME") => "anime",
        Some("MANGA") => match format.as_deref() {
            Some("novel") => "novel",
            _ => "manga",
        },
        _ => match format.as_deref() {
            Some("novel") => "novel",
            Some("one_shot" | "manga") => "manga",
            Some(_) => "anime",
            None => "anime",
        },
    };
    (content_type, format)
}

fn pub_status(value: Option<&str>) -> Option<String> {
    match value {
        Some("FINISHED") => Some("completed".to_string()),
        Some("RELEASING") => Some("ongoing".to_string()),
        Some("NOT_YET_RELEASED") => Some("announced".to_string()),
        Some("HIATUS") => Some("hiatus".to_string()),
        Some("CANCELLED") => Some("cancelled".to_string()),
        _ => None,
    }
}

/// Entry status → `CoreStatus::as_str()` values.
fn list_status(value: Option<&str>) -> Option<String> {
    match value {
        Some("CURRENT") => Some("in_progress".to_string()),
        Some("COMPLETED") => Some("completed".to_string()),
        Some("PLANNING") => Some("planned".to_string()),
        Some("PAUSED") => Some("on_hold".to_string()),
        Some("DROPPED") => Some("dropped".to_string()),
        Some("REPEATING") => Some("repeat".to_string()),
        _ => None,
    }
}

/// AniList scores are 0–100; we normalize to the app's 0–10 integer scale.
fn score_to_rating(value: Option<&Value>) -> Option<String> {
    let score = match value {
        Some(Value::Number(number)) => number.as_i64(),
        Some(Value::String(text)) => text.trim().parse::<i64>().ok(),
        _ => None,
    }?;
    if score <= 0 {
        None
    } else {
        Some((score / 10).to_string())
    }
}

/// `{ "year": y, "month": m, "day": d }` → `YYYY-MM-DD` (missing parts → 1).
fn anilist_date(value: Option<&Value>) -> Option<String> {
    let object = value.and_then(Value::as_object)?;
    let year = object.get("year").and_then(Value::as_i64)?;
    if year <= 0 {
        return None;
    }
    let month = object.get("month").and_then(Value::as_i64).unwrap_or(1);
    let day = object.get("day").and_then(Value::as_i64).unwrap_or(1);
    Some(format!("{year:04}-{month:02}-{day:02}"))
}

/// A `startDate` object → its year (for `release_year`).
fn anilist_year(value: Option<&Value>) -> Option<String> {
    let object = value.and_then(Value::as_object)?;
    let year = object.get("year").and_then(Value::as_i64)?;
    if year <= 0 {
        None
    } else {
        Some(year.to_string())
    }
}

fn staff_people(value: Option<&Value>) -> Vec<(String, String)> {
    let Some(edges) = value.and_then(|v| v.get("edges")).and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for edge in edges {
        let Some(role) = edge.get("role").and_then(Value::as_str) else {
            continue;
        };
        let Some(name) = edge
            .get("node")
            .and_then(|node| node.get("name"))
            .and_then(|name| name.get("full"))
            .and_then(Value::as_str)
        else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        let role_lower = role.to_ascii_lowercase();
        let person_role = if role_lower.contains("story") {
            Some("author")
        } else if role_lower.contains("art") {
            Some("artist")
        } else if role_lower.contains("director") {
            Some("director")
        } else if role_lower.contains("studio") {
            Some("studio")
        } else {
            None
        };
        if let Some(person_role) = person_role {
            out.push((person_role.to_string(), name.to_string()));
        }
    }
    out
}

fn tag_names(value: Option<&Value>) -> Vec<String> {
    let Some(tags) = value.and_then(Value::as_array) else {
        return Vec::new();
    };
    tags.iter()
        .filter_map(|tag| tag.get("name").and_then(Value::as_str))
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .collect()
}

/// The media's own AniList id plus its MAL id (the identity backbone).
fn external_ids(media: &Value) -> Vec<(String, String, Option<String>)> {
    let mut out = Vec::new();
    if let Some(id) = media.get("id").and_then(Value::as_i64) {
        out.push((
            "anilist".to_string(),
            id.to_string(),
            Some(format!("https://anilist.co/anime/{id}")),
        ));
    }
    if let Some(mal_id) = media.get("idMal").and_then(Value::as_i64) {
        out.push(("mal".to_string(), mal_id.to_string(), None));
    }
    out
}

fn cover_url(value: Option<&Value>) -> Option<String> {
    for key in ["extraLarge", "large", "medium"] {
        if let Some(url) = value
            .and_then(|v| v.get(key))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|url| !url.is_empty())
        {
            return Some(url.to_string());
        }
    }
    None
}

fn opt_string(object: &Value, key: &str) -> Option<String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn opt_number(object: &Value, key: &str) -> Option<String> {
    match object.get(key) {
        Some(Value::Number(number)) => number.as_i64().filter(|n| *n > 0).map(|n| n.to_string()),
        _ => None,
    }
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Strip the HTML AniList descriptions carry (`<br>`, `<i>`, …).
fn strip_html(value: String) -> String {
    let mut out = String::with_capacity(value.len());
    let mut in_tag = false;
    for ch in value.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn export_source() -> &'static str {
        r#"{
          "mediaListCollection": {
            "lists": [
              { "name": "Anime", "entries": [
                {
                  "media": {
                    "id": 100,
                    "idMal": 200,
                    "title": { "romaji": "Sword of the Dawn", "english": "Dawn's Sword", "native": "夜明けの剣" },
                    "type": "MANGA",
                    "format": "NOVEL",
                    "status": "RELEASING",
                    "startDate": { "year": 2025, "month": 1, "day": 1 },
                    "endDate": { "year": null, "month": null, "day": null },
                    "chapters": 40,
                    "duration": null,
                    "coverImage": { "extraLarge": "https://cdn.example/cover.jpg" },
                    "bannerImage": "https://cdn.example/banner.jpg",
                    "genres": ["Fantasy", "Adventure"],
                    "tags": [{ "name": "Isekai", "rank": 85 }],
                    "staff": { "edges": [
                      { "node": { "name": { "full": "Jane" } }, "role": "Story" },
                      { "node": { "name": { "full": "Art Co." } }, "role": "Art" }
                    ] },
                    "description": "<b>Synopsis.</b><br>More."
                  },
                  "status": "CURRENT",
                  "score": 85,
                  "progress": 12,
                  "repeat": 0,
                  "notes": "Lovely.",
                  "startedAt": { "year": 2026, "month": 1, "day": 5 },
                  "completedAt": { "year": null, "month": null, "day": null }
                },
                {
                  "media": {
                    "id": 101,
                    "idMal": null,
                    "title": { "romaji": "Berserk", "english": null, "native": null },
                    "type": "MANGA",
                    "format": "MANGA",
                    "status": "FINISHED",
                    "startDate": { "year": 1989 },
                    "endDate": null,
                    "chapters": 375
                  },
                  "status": "COMPLETED",
                  "score": 100,
                  "progress": 375,
                  "startedAt": { "year": 2020, "month": 3 },
                  "completedAt": { "year": 2026, "month": 8, "day": 17 }
                }
              ] }
            ]
          }
        }"#
    }

    #[test]
    fn parses_entries_into_items_with_user_state() {
        let items = AniListParser.parse(export_source()).expect("valid");
        assert_eq!(items.len(), 2);

        let first = &items[0];
        assert_eq!(first.row, 1);
        assert_eq!(first.title, "Sword of the Dawn");
        assert_eq!(first.title_original.as_deref(), Some("夜明けの剣"));
        assert_eq!(first.alt_titles, vec!["Dawn's Sword"]);
        assert_eq!(
            first.content_type.as_deref(),
            Some("novel"),
            "MANGA+NOVEL → novel"
        );
        assert_eq!(first.format.as_deref(), Some("novel"));
        assert_eq!(first.pub_status.as_deref(), Some("ongoing"));
        assert_eq!(first.start_date.as_deref(), Some("2025-01-01"));
        assert_eq!(first.release_year.as_deref(), Some("2025"));
        assert_eq!(first.end_date, None);
        assert_eq!(first.ch_count.as_deref(), Some("40"));
        assert_eq!(first.genres, vec!["Fantasy", "Adventure"]);
        assert_eq!(first.tags, vec!["Isekai"]);
        assert_eq!(
            first.people,
            vec![
                ("author".to_string(), "Jane".to_string()),
                ("artist".to_string(), "Art Co.".to_string()),
            ]
        );
        assert_eq!(first.synopsis.as_deref(), Some("Synopsis.More."));
        assert_eq!(
            first.external_ids,
            vec![
                (
                    "anilist".to_string(),
                    "100".to_string(),
                    Some("https://anilist.co/anime/100".to_string())
                ),
                ("mal".to_string(), "200".to_string(), None),
            ]
        );
        assert_eq!(
            first.cover_url.as_deref(),
            Some("https://cdn.example/cover.jpg")
        );
        assert_eq!(
            first.banner_url.as_deref(),
            Some("https://cdn.example/banner.jpg")
        );

        assert_eq!(first.my_status.as_deref(), Some("in_progress"));
        assert_eq!(first.my_rating.as_deref(), Some("8"), "85/10 → 8");
        assert_eq!(first.progress.as_deref(), Some("12"));
        assert_eq!(first.repeat_count, None, "zero repeat stays absent");
        assert_eq!(first.started_at.as_deref(), Some("2026-01-05"));
        assert_eq!(first.completed_at, None);
        assert_eq!(first.my_review.as_deref(), Some("Lovely."));

        let second = &items[1];
        assert_eq!(second.title, "Berserk");
        assert_eq!(second.content_type.as_deref(), Some("manga"));
        assert_eq!(second.pub_status.as_deref(), Some("completed"));
        assert_eq!(
            second.start_date.as_deref(),
            Some("1989-01-01"),
            "year-only → Jan 1"
        );
        assert_eq!(second.my_status.as_deref(), Some("completed"));
        assert_eq!(second.my_rating.as_deref(), Some("10"));
        assert_eq!(second.progress.as_deref(), Some("375"));
        assert_eq!(
            second.started_at.as_deref(),
            Some("2020-03-01"),
            "missing day → 1"
        );
        assert_eq!(second.completed_at.as_deref(), Some("2026-08-17"));
        assert_eq!(
            second.external_ids,
            vec![(
                "anilist".to_string(),
                "101".to_string(),
                Some("https://anilist.co/anime/101".to_string())
            )],
            "null idMal is skipped"
        );
    }

    #[test]
    fn skips_entries_without_media_and_keeps_row_numbers() {
        let source = r#"{"mediaListCollection":{"lists":[
            {"entries":[{"media":{"id":1,"title":{"romaji":"A"},"type":"ANIME"}}]},
            {"entries":[{},{"media":{"id":2,"title":{"romaji":"B"},"type":"ANIME"}}]}
        ]}}"#;
        let items = AniListParser.parse(source).expect("valid");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "A");
        assert_eq!(items[1].title, "B");
        assert_eq!(items[1].row, 2);
    }

    #[test]
    fn rejects_non_anilist_json() {
        let error = AniListParser
            .parse(r#"[{"title":"Sword"}]"#)
            .expect_err("array");
        assert!(matches!(error, ImportError::Parse(_)));

        let error = AniListParser
            .parse(r#"{"mediaListCollection":{"lists":{}}}"#)
            .expect_err("lists not array");
        assert!(matches!(error, ImportError::Parse(_)));
    }

    #[test]
    fn malformed_json_is_a_parse_error() {
        let error = AniListParser.parse("not json").expect_err("malformed");
        assert!(matches!(error, ImportError::Parse(_)));
    }
}
