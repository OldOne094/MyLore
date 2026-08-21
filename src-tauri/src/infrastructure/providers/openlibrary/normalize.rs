//! Pure OpenLibrary → domain mappers (MISSION-057).
//!
//! Books have no chapter tree, no relations and no "episodes", so this module
//! only maps candidates (search), media (work + authors) and external ids
//! (first edition). The HTML sanitizer is a local copy shared across adapters.

use crate::domain::enums::{ContentType, MediaStatus, PersonRole};
use crate::domain::provider::types::{ProviderCandidate, ProviderMedia, ProviderPerson};
use crate::domain::value_objects::{ExternalId, ProviderId};

use super::response::{Description, Edition, SearchDoc, WorkResponse};
use super::PROVIDER_ID;

/// Cap the number of subjects surfaced as genres per title.
const MAX_GENRES: usize = 8;

/// Strip HTML tags and collapse whitespace (OpenLibrary `description` is
/// sometimes a plain string, sometimes plain text — never HTML, but works
/// everywhere). Local copy shared across adapters.
fn strip_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;
    for c in input.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// OpenLibrary only catalogs books (editions of works).
pub(crate) fn content_type() -> ContentType {
    ContentType::Book
}

/// A work with a known first-publish date is a published (completed) book.
pub(crate) fn pub_status(first_publish_date: Option<&str>) -> MediaStatus {
    if first_publish_date.is_some() {
        MediaStatus::Completed
    } else {
        MediaStatus::Unknown
    }
}

/// `"/works/OL89650W"` → `"OL89650W"`. The provider id is the bare work key;
/// both the full key and the bare id round-trip.
pub(crate) fn work_id(key: &str) -> String {
    key.rsplit('/').next().unwrap_or(key).to_string()
}

/// `covers.id` → cover CDN url (medium size, `M`).
pub(crate) fn cover_url(cover_id: i64) -> String {
    format!("https://covers.openlibrary.org/b/id/{cover_id}-M.jpg")
}

/// Canonical human-facing page for a work.
pub(crate) fn page_url(work_id: &str) -> String {
    format!("https://openlibrary.org/works/{work_id}")
}

/// Extract a 4-digit year from OpenLibrary's free-form dates: `"1965"`,
/// `"May 1989"`, `"1989-05-15"` → the first run of 4 digits.
pub(crate) fn year_from_date(date: Option<&str>) -> Option<i32> {
    let mut buf = String::new();
    for c in date?.chars() {
        if c.is_ascii_digit() {
            buf.push(c);
            if buf.len() == 4 {
                return buf.parse().ok();
            }
        } else {
            buf.clear();
        }
    }
    None
}

fn description_text(description: Option<&Description>) -> Option<String> {
    let text = match description? {
        Description::Text(s) => s.clone(),
        Description::Value(v) => v.value.clone()?,
    };
    let clean = strip_html(&text);
    if clean.is_empty() {
        None
    } else {
        Some(clean)
    }
}

/// A search row → candidate. `None` when the row lacks a resolvable id/title.
pub(crate) fn candidate(doc: &SearchDoc) -> Option<ProviderCandidate> {
    let id = work_id(doc.key.as_deref()?);
    Some(ProviderCandidate {
        provider: PROVIDER_ID.to_string(),
        provider_id: id.clone(),
        title: doc.title.clone()?,
        content_type: content_type(),
        release_year: doc.first_publish_year.map(|y| y as i32),
        cover_url: doc.cover_i.map(cover_url),
        synopsis: None, // search rows carry no description
        external_ids: Vec::new(),
        url: Some(page_url(&id)),
    })
}

/// A resolved work + its authors → full `ProviderMedia`.
pub(crate) fn media(work: &WorkResponse, authors: Vec<ProviderPerson>) -> ProviderMedia {
    let id = work_id(work.key.as_deref().unwrap_or_default());
    ProviderMedia {
        provider: PROVIDER_ID.to_string(),
        provider_id: id.clone(),
        title_main: work.title.clone().unwrap_or_default(),
        title_original: None,
        alt_titles: Vec::new(),
        content_type: content_type(),
        format: None,
        pub_status: pub_status(work.first_publish_date.as_deref()),
        synopsis: description_text(work.description.as_ref()),
        start_date: None,
        end_date: None,
        release_year: year_from_date(work.first_publish_date.as_deref()),
        language: None,
        country: None,
        content_rating: None,
        pages: None, // page counts live on editions, not the work
        duration_min: None,
        ep_count: None,
        ch_count: None,
        cover_url: work
            .covers
            .as_ref()
            .and_then(|c| c.first())
            .copied()
            .map(cover_url),
        banner_url: None,
        url: Some(page_url(&id)),
        people: authors,
        genres: work
            .subjects
            .as_deref()
            .unwrap_or_default()
            .iter()
            .take(MAX_GENRES)
            .cloned()
            .collect(),
        tags: Vec::new(),
        external_ids: Vec::new(),
    }
}

/// The first edition's ISBNs/LCCN/OCLC → external ids (used for dedup).
/// Validation failures are skipped (blank values, invalid provider ids).
pub(crate) fn external_ids(edition: &Edition) -> Vec<ExternalId> {
    let mut ids = Vec::new();
    for (provider, values) in [
        ("isbn10", edition.isbn_10.as_deref().unwrap_or_default()),
        ("isbn13", edition.isbn_13.as_deref().unwrap_or_default()),
        ("lccn", edition.lccn.as_deref().unwrap_or_default()),
        ("oclc", edition.oclc_numbers.as_deref().unwrap_or_default()),
    ] {
        for value in values {
            push_external(&mut ids, provider, value);
        }
    }
    ids
}

fn push_external(out: &mut Vec<ExternalId>, provider: &str, value: &str) {
    if let Ok(provider) = ProviderId::new(provider) {
        if let Ok(external) = ExternalId::new(provider, value, None) {
            out.push(external);
        }
    }
}

/// Flatten resolved author names into `PersonRole::Author` credits.
pub(crate) fn authors(names: &[String]) -> Vec<ProviderPerson> {
    names
        .iter()
        .filter(|n| !n.trim().is_empty())
        .map(|name| ProviderPerson {
            role: PersonRole::Author,
            name: name.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::from_str;

    use super::*;
    use crate::infrastructure::providers::test_support::fixture;

    fn search_doc() -> SearchDoc {
        let data: super::super::response::SearchResponse =
            from_str(&fixture("openlibrary", "search_books.json")).unwrap();
        data.docs.first().unwrap().clone()
    }

    #[test]
    fn work_id_strips_the_path_prefix() {
        assert_eq!(work_id("/works/OL89650W"), "OL89650W");
        assert_eq!(work_id("OL89650W"), "OL89650W");
    }

    #[test]
    fn candidate_maps_search_row() {
        let c = candidate(&search_doc()).unwrap();
        assert_eq!(c.provider, "openlibrary");
        assert_eq!(c.provider_id, "OL89650W");
        assert_eq!(c.title, "Dune");
        assert_eq!(c.content_type, ContentType::Book);
        assert_eq!(c.release_year, Some(1965));
        assert_eq!(
            c.cover_url.as_deref(),
            Some("https://covers.openlibrary.org/b/id/68486-M.jpg")
        );
        assert!(c.synopsis.is_none());
        assert_eq!(
            c.url.as_deref(),
            Some("https://openlibrary.org/works/OL89650W")
        );
    }

    #[test]
    fn candidate_drops_rows_without_a_title() {
        let mut doc = search_doc();
        doc.title = None;
        assert!(candidate(&doc).is_none());
    }

    #[test]
    fn year_from_date_handles_free_form() {
        assert_eq!(year_from_date(Some("1965")), Some(1965));
        assert_eq!(year_from_date(Some("May 1989")), Some(1989));
        assert_eq!(year_from_date(Some("1989-05-15")), Some(1989));
        assert_eq!(year_from_date(None), None);
    }

    #[test]
    fn media_maps_work_and_authors() {
        let work: WorkResponse = from_str(&fixture("openlibrary", "work.json")).unwrap();
        let m = media(&work, authors(&["Frank Herbert".into()]));
        assert_eq!(m.provider_id, "OL89650W");
        assert_eq!(m.title_main, "Dune");
        assert_eq!(m.content_type, ContentType::Book);
        assert_eq!(m.pub_status, MediaStatus::Completed);
        assert_eq!(m.release_year, Some(1965));
        assert_eq!(
            m.synopsis.as_deref(),
            Some("Set on the desert planet Arrakis, Dune is the story of Paul Atreides.")
        );
        assert_eq!(
            m.cover_url.as_deref(),
            Some("https://covers.openlibrary.org/b/id/68486-M.jpg")
        );
        assert_eq!(
            m.people,
            vec![ProviderPerson {
                role: PersonRole::Author,
                name: "Frank Herbert".into(),
            }]
        );
        assert!(m.genres.iter().any(|g| g == "Science fiction"));
        assert!(m.genres.len() <= MAX_GENRES);
        assert!(m.pages.is_none());
    }

    #[test]
    fn description_string_variant_parses() {
        let work: WorkResponse =
            serde_json::from_str(r#"{"title":"Plain","description":"Just a string","authors":[]}"#)
                .unwrap();
        assert_eq!(
            work.description
                .as_ref()
                .map(|d| description_text(Some(d)).unwrap()),
            Some("Just a string".into())
        );
    }

    #[test]
    fn external_ids_flatten_edition_numbers() {
        let ed: Edition = serde_json::from_str(
            r#"{
                "title":"Dune",
                "isbn_10":["0441172717"],
                "isbn_13":["9780441172719"],
                "lccn":["65020016"],
                "oclc_numbers":["5752382"]
            }"#,
        )
        .unwrap();
        let ids = external_ids(&ed);
        let find = |p: &str| ids.iter().find(|e| e.provider().as_str() == p);
        assert_eq!(find("isbn10").unwrap().value(), "0441172717");
        assert_eq!(find("isbn13").unwrap().value(), "9780441172719");
        assert_eq!(find("lccn").unwrap().value(), "65020016");
        assert_eq!(find("oclc").unwrap().value(), "5752382");
        assert_eq!(ids.len(), 4);
    }
}
