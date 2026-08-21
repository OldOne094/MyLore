//! Pure Hardcover → domain mappers (MISSION-064).
//!
//! The optional third book provider: same `Book` domain as OpenLibrary/Google
//! Books, so the coordinator's parallel fan-out gets yet another live fallback.
//! Hardcover also distinguishes light/web novels via `book_category_id`, which
//! is only available on details — search blobs have no category, so candidates
//! normalize as `Book` and details re-derive the true type.

use serde_json::Value;

use crate::domain::enums::{ContentType, MediaStatus, PersonRole};
use crate::domain::provider::types::{ProviderCandidate, ProviderMedia, ProviderPerson};
use crate::domain::value_objects::{ExternalId, ProviderId};

use super::response::{Book, SearchBook};
use super::PROVIDER_ID;

/// Cap the number of genres surfaced per title.
const MAX_GENRES: usize = 8;

/// Search hits carry no category in the Typesense blob → always a Book. The
/// true type is re-derived on details via `book_category_id`.
pub(crate) fn search_content_type() -> ContentType {
    ContentType::Book
}

/// Hardcover book category → domain content type (API_PROVIDERS §12):
/// 1 Book, 2 Novella, 9 Web Novel, 10 Light Novel. Everything else defaults to
/// Book.
pub(crate) fn content_type(book_category_id: Option<i64>) -> ContentType {
    match book_category_id {
        Some(9) => ContentType::WebNovel,
        Some(2) | Some(10) => ContentType::Novel,
        _ => ContentType::Book,
    }
}

/// A known release date → completed book.
pub(crate) fn pub_status(release_date: Option<&str>) -> MediaStatus {
    match release_date {
        Some(d) if !d.trim().is_empty() => MediaStatus::Completed,
        _ => MediaStatus::Unknown,
    }
}

/// `release_year` when present, else the first 4-digit run in `release_date`.
pub(crate) fn release_year(release_year: Option<i64>, release_date: Option<&str>) -> Option<i32> {
    if let Some(year) = release_year {
        return Some(year as i32);
    }
    year_from_date(release_date)
}

fn year_from_date(release_date: Option<&str>) -> Option<i32> {
    let mut buf = String::new();
    for c in release_date?.chars() {
        if c.is_ascii_digit() {
            buf.push(c);
            if buf.len() == 4 {
                let year: i32 = buf.parse().ok()?;
                return if year == 0 { None } else { Some(year) };
            }
        } else {
            buf.clear();
        }
    }
    None
}

fn hardcover_url(slug: Option<&str>) -> Option<String> {
    slug.filter(|s| !s.trim().is_empty())
        .map(|s| format!("https://hardcover.app/books/{s}"))
}

/// Collapse whitespace/newlines on synopsis/description text.
fn clean_text(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `cached_tags` (jsonb) buckets tags by category; the `Genre` bucket maps to
/// domain genres.
pub(crate) fn genres_from_cached_tags(cached_tags: Option<&Value>) -> Vec<String> {
    let Some(genres) = cached_tags
        .and_then(|tags| tags.get("Genre"))
        .and_then(|g| g.as_array())
    else {
        return Vec::new();
    };
    genres
        .iter()
        .filter_map(|g| g.as_str())
        .filter(|g| !g.trim().is_empty())
        .take(MAX_GENRES)
        .map(ToOwned::to_owned)
        .collect()
}

/// A search blob → candidate. Rows without an id or a non-blank title drop.
pub(crate) fn candidate(row: &SearchBook) -> Option<ProviderCandidate> {
    let id = row.id.clone()?;
    let title = row.title.as_deref()?;
    if title.trim().is_empty() {
        return None;
    }
    Some(ProviderCandidate {
        provider: PROVIDER_ID.to_string(),
        provider_id: id,
        title: title.to_string(),
        content_type: search_content_type(),
        release_year: row.release_year.map(|y| y as i32),
        cover_url: None, // the search blob carries no cover URL
        synopsis: row
            .description
            .as_deref()
            .map(clean_text)
            .filter(|s| !s.is_empty()),
        external_ids: Vec::new(),
        url: hardcover_url(row.slug.as_deref()),
    })
}

/// A full book → `ProviderMedia`.
pub(crate) fn media(book: &Book) -> Option<ProviderMedia> {
    let title = book.title.as_deref()?;
    if title.trim().is_empty() {
        return None;
    }
    let mut alt_titles = Vec::new();
    if let Some(subtitle) = book.subtitle.as_deref().filter(|s| !s.trim().is_empty()) {
        alt_titles.push(subtitle.to_string());
    }
    Some(ProviderMedia {
        provider: PROVIDER_ID.to_string(),
        provider_id: book.id.to_string(),
        title_main: title.to_string(),
        title_original: None,
        alt_titles,
        content_type: content_type(book.book_category_id),
        format: None,
        pub_status: pub_status(book.release_date.as_deref()),
        synopsis: book
            .description
            .as_deref()
            .map(clean_text)
            .filter(|s| !s.is_empty()),
        start_date: book.release_date.clone(),
        end_date: None,
        release_year: release_year(book.release_year, book.release_date.as_deref()),
        language: None,
        country: None,
        content_rating: None,
        pages: book.pages.map(|n| n.max(0) as u32),
        duration_min: None,
        ep_count: None,
        ch_count: None,
        cover_url: book
            .image
            .as_ref()
            .and_then(|image| image.url.clone())
            .filter(|u| !u.trim().is_empty()),
        banner_url: None,
        url: hardcover_url(book.slug.as_deref()),
        people: authors(book),
        genres: genres_from_cached_tags(book.cached_tags.as_ref()),
        tags: Vec::new(),
        external_ids: external_ids(book),
    })
}

/// Edition ISBNs → isbn10/isbn13 external ids (deduped).
pub(crate) fn external_ids(book: &Book) -> Vec<ExternalId> {
    let mut ids: Vec<ExternalId> = Vec::new();
    for edition in book.editions.as_deref().unwrap_or_default() {
        for (provider, value) in [
            ("isbn10", edition.isbn_10.as_deref()),
            ("isbn13", edition.isbn_13.as_deref()),
        ] {
            let Some(value) = value.filter(|v| !v.trim().is_empty()) else {
                continue;
            };
            let Ok(provider_id) = ProviderId::new(provider) else {
                continue;
            };
            let Ok(id) = ExternalId::new(provider_id, value, None) else {
                continue;
            };
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
    }
    ids
}

/// Contributions → `PersonRole::Author` credits (deduped per name).
pub(crate) fn authors(book: &Book) -> Vec<ProviderPerson> {
    let mut out: Vec<ProviderPerson> = Vec::new();
    for contribution in book.contributions.as_deref().unwrap_or_default() {
        let Some(name) = contribution
            .author
            .as_ref()
            .and_then(|author| author.name.clone())
        else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        let person = ProviderPerson {
            role: PersonRole::Author,
            name: name.to_string(),
        };
        if !out.contains(&person) {
            out.push(person);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use serde_json::from_str;

    use super::*;
    use crate::infrastructure::providers::test_support::fixture;

    fn search_rows() -> Vec<SearchBook> {
        let envelope: super::super::response::Envelope<super::super::response::SearchPayload> =
            from_str(&fixture("hardcover", "search_books.json")).unwrap();
        let payload = envelope.data.unwrap();
        payload
            .search
            .results
            .iter()
            .map(|value| serde_json::from_value(value.clone()).unwrap())
            .collect()
    }

    fn dune_book() -> Book {
        let envelope: super::super::response::Envelope<super::super::response::BooksData> =
            from_str(&fixture("hardcover", "book_details.json")).unwrap();
        let data = envelope.data.unwrap();
        data.books[0].clone()
    }

    #[test]
    fn search_book_accepts_numeric_and_string_ids() {
        let string_id: SearchBook =
            serde_json::from_str(r#"{"id": "3342", "title": "Dune"}"#).unwrap();
        assert_eq!(string_id.id.as_deref(), Some("3342"));
        let numeric_id: SearchBook =
            serde_json::from_str(r#"{"id": 3342, "title": "Dune"}"#).unwrap();
        assert_eq!(numeric_id.id.as_deref(), Some("3342"));
        let missing_id: SearchBook = serde_json::from_str(r#"{"title": "Dune"}"#).unwrap();
        assert_eq!(missing_id.id, None);
    }

    #[test]
    fn candidate_maps_search_row() {
        let row = &search_rows()[0];
        let c = candidate(row).unwrap();
        assert_eq!(c.provider, "hardcover");
        assert_eq!(c.provider_id, "3342");
        assert_eq!(c.title, "Dune");
        assert_eq!(c.content_type, ContentType::Book);
        assert_eq!(c.release_year, Some(1965));
        assert!(c
            .synopsis
            .as_deref()
            .is_some_and(|s| s.starts_with("Set on the desert planet")));
        assert_eq!(c.url.as_deref(), Some("https://hardcover.app/books/dune"));
        assert!(c.cover_url.is_none());
    }

    #[test]
    fn candidate_drops_rows_without_id_or_title() {
        let mut rows = search_rows();
        assert_eq!(candidate(&rows[1]).unwrap().title, "Dune Messiah");
        rows[0].id = None;
        assert!(candidate(&rows[0]).is_none());
        assert!(candidate(&rows[2]).is_none(), "blank title drops");
    }

    #[test]
    fn media_maps_book() {
        let m = media(&dune_book()).unwrap();
        assert_eq!(m.provider_id, "3342");
        assert_eq!(m.title_main, "Dune");
        assert_eq!(m.content_type, ContentType::Book);
        assert_eq!(m.pub_status, MediaStatus::Completed);
        assert_eq!(m.release_year, Some(1965));
        assert_eq!(m.start_date.as_deref(), Some("1965-08-01"));
        assert_eq!(m.pages, Some(704));
        assert_eq!(m.people.len(), 1);
        assert_eq!(m.people[0].name, "Frank Herbert");
        assert_eq!(m.people[0].role, PersonRole::Author);
        assert!(m.genres.iter().any(|g| g == "Science Fiction"));
        assert!(
            !m.genres.iter().any(|g| g == "Adventurous"),
            "mood bucket is not a genre"
        );
        assert_eq!(
            m.cover_url.as_deref(),
            Some("https://images.hardcover.app/v1/cover/dune.jpg")
        );
        assert_eq!(m.url.as_deref(), Some("https://hardcover.app/books/dune"));
    }

    #[test]
    fn content_type_maps_book_category() {
        assert_eq!(content_type(Some(1)), ContentType::Book);
        assert_eq!(content_type(Some(2)), ContentType::Novel);
        assert_eq!(content_type(Some(9)), ContentType::WebNovel);
        assert_eq!(content_type(Some(10)), ContentType::Novel);
        assert_eq!(content_type(None), ContentType::Book);
        assert_eq!(content_type(Some(99)), ContentType::Book);
    }

    #[test]
    fn release_year_prefers_field_over_date() {
        assert_eq!(release_year(Some(1965), Some("2000-01-01")), Some(1965));
        assert_eq!(release_year(None, Some("1969-06-01")), Some(1969));
        assert_eq!(release_year(None, None), None);
    }

    #[test]
    fn external_ids_uses_edition_isbns_and_dedupes() {
        let ids = external_ids(&dune_book());
        let values: Vec<String> = ids
            .iter()
            .map(|e| format!("{}:{}", e.provider().as_str(), e.value()))
            .collect();
        assert!(values.iter().any(|v| v == "isbn10:0441172717"));
        assert!(values.iter().any(|v| v == "isbn13:9780441172719"));
        assert!(values.iter().any(|v| v == "isbn13:9780593099322"));
        assert_eq!(ids.len(), 3, "deduped across editions");
    }

    #[test]
    fn authors_dedupe_repeated_credits() {
        let mut book = dune_book();
        book.contributions = Some(vec![
            crate::infrastructure::providers::hardcover::response::Contribution {
                author: Some(
                    crate::infrastructure::providers::hardcover::response::Author {
                        name: Some("Frank Herbert".to_string()),
                    },
                ),
            },
            crate::infrastructure::providers::hardcover::response::Contribution {
                author: Some(
                    crate::infrastructure::providers::hardcover::response::Author {
                        name: Some("Frank Herbert".to_string()),
                    },
                ),
            },
        ]);
        assert_eq!(authors(&book).len(), 1);
    }

    #[test]
    fn light_novel_category_normalizes_to_novel() {
        let mut book = dune_book();
        book.book_category_id = Some(10);
        assert_eq!(media(&book).unwrap().content_type, ContentType::Novel);
    }
}
