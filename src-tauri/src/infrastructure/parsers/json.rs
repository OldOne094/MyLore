//! The MyLore JSON import format (MISSION-068, ARCHITECTURE §6).
//!
//! A file is a **top-level array of item objects**. Every field is optional —
//! a missing field maps to `None` and the pipeline's validator/normalizer
//! decide how to degrade the row. Counts and years accept numbers **or**
//! strings (both map to the `String`-ly parsed `ParsedItem`). Structural
//! problems (not an array, an element that is not an object) abort the whole
//! file with `ImportError::Parse`; content problems are per-row.
//!
//! ```json
//! [
//!   {
//!     "title": "Sword of the Dawn",
//!     "title_original": "夜明けの剣",
//!     "alt_titles": ["Dawn's Sword"],
//!     "content_type": "novel",
//!     "format": "light_novel",
//!     "pub_status": "ongoing",
//!     "start_date": "2025-01-01",
//!     "release_year": 2025,
//!     "language": "ja",
//!     "country": "JP",
//!     "pages": 320,
//!     "synopsis": "…",
//!     "people": [{ "role": "author", "name": "Test Author" }],
//!     "genres": ["Fantasy"],
//!     "tags": ["isekai"],
//!     "external_ids": [{ "provider": "anilist", "value": "42", "url": null }],
//!     "cover_url": "https://…",
//!     "banner_url": "https://…"
//!   }
//! ]
//! ```

use serde_json::{Map, Value};

use crate::domain::import::{ImportError, ImportParser, ParsedItem};

/// Parses the MyLore JSON import format into pipeline items.
pub struct JsonParser;

impl ImportParser for JsonParser {
    fn parse(&self, source: &str) -> Result<Vec<ParsedItem>, ImportError> {
        let document: Value = serde_json::from_str(source)
            .map_err(|error| ImportError::Parse(format!("invalid JSON: {error}")))?;
        let array = document.as_array().ok_or_else(|| {
            ImportError::Parse(
                "expected a top-level array of items (the MyLore JSON import format)".to_string(),
            )
        })?;

        let mut items = Vec::with_capacity(array.len());
        for (index, element) in array.iter().enumerate() {
            let object = element.as_object().ok_or_else(|| {
                ImportError::Parse(format!("element {} is not an object", index + 1))
            })?;
            items.push(to_item(index + 1, object));
        }
        Ok(items)
    }
}

fn to_item(row: usize, object: &Map<String, Value>) -> ParsedItem {
    ParsedItem {
        row,
        title: opt_string(object, "title").unwrap_or_default(),
        title_original: opt_string(object, "title_original"),
        alt_titles: string_list(object, "alt_titles"),
        content_type: opt_string(object, "content_type"),
        format: opt_string(object, "format"),
        pub_status: opt_string(object, "pub_status"),
        start_date: opt_string(object, "start_date"),
        end_date: opt_string(object, "end_date"),
        release_year: opt_string(object, "release_year"),
        language: opt_string(object, "language"),
        country: opt_string(object, "country"),
        content_rating: opt_string(object, "content_rating"),
        pages: opt_string(object, "pages"),
        duration_min: opt_string(object, "duration_min"),
        ep_count: opt_string(object, "ep_count"),
        ch_count: opt_string(object, "ch_count"),
        synopsis: opt_string(object, "synopsis"),
        people: people(object.get("people")),
        genres: string_list(object, "genres"),
        tags: string_list(object, "tags"),
        external_ids: external_ids(object.get("external_ids")),
        cover_url: opt_string(object, "cover_url"),
        banner_url: opt_string(object, "banner_url"),
    }
}

/// A scalar field: string (trimmed, blank → None) or number (stringified).
fn opt_string(object: &Map<String, Value>, key: &str) -> Option<String> {
    match object.get(key) {
        Some(Value::String(value)) => {
            let value = value.trim();
            if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            }
        }
        Some(Value::Number(number)) => Some(number.to_string()),
        _ => None,
    }
}

/// A list field: an array of strings, or a single string treated as one entry.
fn string_list(object: &Map<String, Value>, key: &str) -> Vec<String> {
    match object.get(key) {
        Some(Value::Array(values)) => values
            .iter()
            .filter_map(|value| match value {
                Value::String(value) => {
                    let value = value.trim();
                    if value.is_empty() {
                        None
                    } else {
                        Some(value.to_string())
                    }
                }
                _ => None,
            })
            .collect(),
        Some(Value::String(value)) => {
            let value = value.trim();
            if value.is_empty() {
                Vec::new()
            } else {
                vec![value.to_string()]
            }
        }
        _ => Vec::new(),
    }
}

/// People entries: `{ "role": "author", "name": "Jane" }` or a bare string
/// (role defaults to `author`).
fn people(value: Option<&Value>) -> Vec<(String, String)> {
    let Some(Value::Array(entries)) = value else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries {
        match entry {
            Value::Object(object) => {
                let Some(name) = object.get("name").and_then(Value::as_str) else {
                    continue;
                };
                let name = name.trim();
                if name.is_empty() {
                    continue;
                }
                let role = object
                    .get("role")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|role| !role.is_empty())
                    .unwrap_or("author");
                out.push((role.to_string(), name.to_string()));
            }
            Value::String(name) => {
                let name = name.trim();
                if !name.is_empty() {
                    out.push(("author".to_string(), name.to_string()));
                }
            }
            _ => {}
        }
    }
    out
}

/// External id entries: `{ "provider": "anilist", "value": "42", "url": "…" }`
/// (`url` optional; entries without a non-blank provider or value are dropped).
fn external_ids(value: Option<&Value>) -> Vec<(String, String, Option<String>)> {
    let Some(Value::Array(entries)) = value else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries {
        let Some(object) = entry.as_object() else {
            continue;
        };
        let Some(provider) = object.get("provider").and_then(Value::as_str) else {
            continue;
        };
        let Some(ext_id) = object.get("value").and_then(Value::as_str) else {
            continue;
        };
        let provider = provider.trim();
        let ext_id = ext_id.trim();
        if provider.is_empty() || ext_id.is_empty() {
            continue;
        }
        let url = object.get("url").and_then(Value::as_str).map(str::trim);
        let url = url.filter(|url| !url.is_empty()).map(str::to_string);
        out.push((provider.to_string(), ext_id.to_string(), url));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_full_array_into_items() {
        let source = r#"[
            {
              "title": "Sword of the Dawn",
              "title_original": "夜明けの剣",
              "alt_titles": ["Dawn's Sword"],
              "content_type": "novel",
              "format": "light_novel",
              "pub_status": "ongoing",
              "start_date": "2025-01-01",
              "end_date": "2025-01-31",
              "release_year": 2025,
              "language": "ja",
              "country": "JP",
              "content_rating": "Teen",
              "pages": 320,
              "duration_min": null,
              "ep_count": null,
              "ch_count": 3,
              "synopsis": "A synopsis.",
              "people": [
                { "role": "author", "name": "Test Author" },
                { "name": "Second Author" },
                "Artist Name"
              ],
              "genres": ["Fantasy", "Adventure"],
              "tags": ["isekai"],
              "external_ids": [
                { "provider": "anilist", "value": "42", "url": "https://anilist.co/42" },
                { "provider": "mal", "value": "1" }
              ],
              "cover_url": "https://cdn.example/cover.jpg",
              "banner_url": "https://cdn.example/banner.jpg"
            },
            { "title": "Berserk", "content_type": "manga", "pages": "380" }
        ]"#;

        let items = JsonParser.parse(source).expect("valid");
        assert_eq!(items.len(), 2);

        let first = &items[0];
        assert_eq!(first.row, 1);
        assert_eq!(first.title, "Sword of the Dawn");
        assert_eq!(first.title_original.as_deref(), Some("夜明けの剣"));
        assert_eq!(first.alt_titles, vec!["Dawn's Sword"]);
        assert_eq!(first.content_type.as_deref(), Some("novel"));
        assert_eq!(first.format.as_deref(), Some("light_novel"));
        assert_eq!(first.pub_status.as_deref(), Some("ongoing"));
        assert_eq!(first.start_date.as_deref(), Some("2025-01-01"));
        assert_eq!(
            first.release_year.as_deref(),
            Some("2025"),
            "number → string"
        );
        assert_eq!(first.language.as_deref(), Some("ja"));
        assert_eq!(first.pages.as_deref(), Some("320"), "number → string");
        assert_eq!(first.ch_count.as_deref(), Some("3"));
        assert_eq!(
            first.people,
            vec![
                ("author".to_string(), "Test Author".to_string()),
                ("author".to_string(), "Second Author".to_string()),
                ("author".to_string(), "Artist Name".to_string()),
            ]
        );
        assert_eq!(first.genres, vec!["Fantasy", "Adventure"]);
        assert_eq!(
            first.external_ids,
            vec![
                (
                    "anilist".to_string(),
                    "42".to_string(),
                    Some("https://anilist.co/42".to_string())
                ),
                ("mal".to_string(), "1".to_string(), None),
            ]
        );

        let second = &items[1];
        assert_eq!(second.row, 2);
        assert_eq!(second.title, "Berserk");
        assert_eq!(
            second.pages.as_deref(),
            Some("380"),
            "string stays a string"
        );
    }

    #[test]
    fn row_numbers_are_one_based() {
        let items = JsonParser
            .parse(r#"[{"title":"A"},{"title":"B"},{"title":"C"}]"#)
            .expect("valid");
        let rows: Vec<usize> = items.iter().map(|item| item.row).collect();
        assert_eq!(rows, vec![1, 2, 3]);
    }

    #[test]
    fn missing_optional_fields_map_to_none_or_empty() {
        let items = JsonParser.parse(r#"[{"title":"Solo"}]"#).expect("valid");
        let item = &items[0];
        assert_eq!(item.title, "Solo");
        assert_eq!(item.content_type, None);
        assert_eq!(item.release_year, None);
        assert!(item.alt_titles.is_empty());
        assert!(item.people.is_empty());
        assert!(item.external_ids.is_empty());
    }

    #[test]
    fn blank_title_surfaces_for_validation() {
        let items = JsonParser
            .parse(r#"[{"title":"  "},{"title":"\t\n"}]"#)
            .expect("valid");
        assert_eq!(items[0].title, "");
        assert_eq!(items[1].title, "");
    }

    #[test]
    fn numbers_stringify_and_wrong_typed_scalars_become_absent() {
        let items = JsonParser
            .parse(r#"[{"title":42,"release_year":{"y":2025},"pages":3.0}]"#)
            .expect("valid");
        let item = &items[0];
        assert_eq!(item.title, "42", "number → string");
        assert_eq!(item.release_year, None, "object year → absent");
        assert_eq!(item.pages.as_deref(), Some("3.0"), "float → string");
    }

    #[test]
    fn non_array_document_is_a_parse_error() {
        let error = JsonParser
            .parse(r#"{"title":"Sword"}"#)
            .expect_err("object, not array");
        assert!(matches!(error, ImportError::Parse(_)));
    }

    #[test]
    fn non_object_element_is_a_parse_error() {
        let error = JsonParser
            .parse(r#"[{"title":"A"},"Sword"]"#)
            .expect_err("string element");
        assert!(matches!(error, ImportError::Parse(_)));
        assert!(error.to_string().contains("element 2"));
    }

    #[test]
    fn malformed_json_is_a_parse_error() {
        let error = JsonParser.parse("not json").expect_err("malformed");
        assert!(matches!(error, ImportError::Parse(_)));
        assert!(error.to_string().contains("invalid JSON"));
    }
}
