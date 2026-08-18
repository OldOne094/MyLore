//! The **Goodreads books CSV export** parser (MISSION-072, ARCHITECTURE §6).
//!
//! Goodreads `My Books` → Import and Export → Export library produces a CSV with
//! a fixed, well-known header (Book Id, Title, Author, ISBN, ISBN13, My Rating,
//! Date Read, Date Added, Bookshelves, Exclusive Shelf, My Review, Number of
//! Pages, Year Published, Original Publication Year, …). The parser reads it
//! with a built-in column mapping — no mapping UI — and feeds the user's list
//! state into the pipeline:
//!
//!   - `Exclusive Shelf` (read / currently-reading / to-read) → `my_status`
//!   - `My Rating` (0–5) → `my_rating` on the 0–10 scale
//!   - `My Review` → `my_review`
//!   - `Date Read` → `completed_at`
//!   - `Bookshelves` → `tags`
//!   - `ISBN13` / `ISBN` → external id provider `isbn`; `Book Id` → `goodreads`
//!
//! Column lookups are case-insensitive (header names lower-cased). A file that
//! is not a Goodreads export still parses; its rows simply lack title/user
//! state and are caught by the pipeline's validator. Structural CSV errors
//! abort with `ImportError::Parse`.

use csv::{ReaderBuilder, StringRecord};

use crate::domain::import::{ImportError, ImportParser, ParsedItem};
use crate::infrastructure::parsers::shared::{self, Columns};

/// Parses the Goodreads library CSV export into pipeline items.
pub struct GoodreadsParser;

impl ImportParser for GoodreadsParser {
    fn parse(&self, source: &str) -> Result<Vec<ParsedItem>, ImportError> {
        let headers = shared::sniff_csv_columns(source, b',')?;
        let columns = Columns::from_headers(&headers);
        let source = source.strip_prefix('\u{feff}').unwrap_or(source);
        let mut reader = ReaderBuilder::new()
            .delimiter(b',')
            .has_headers(false)
            .flexible(true)
            .from_reader(source.as_bytes());

        let mut items = Vec::new();
        for result in reader.records().skip(1) {
            let record = result.map_err(|error| {
                ImportError::Parse(format!("could not read a Goodreads row: {error}"))
            })?;
            items.push(to_item(items.len() + 1, &record, &columns));
        }
        Ok(items)
    }
}

fn to_item(row: usize, record: &StringRecord, columns: &Columns) -> ParsedItem {
    let isbn = columns
        .cell(record, "isbn13")
        .or_else(|| columns.cell(record, "isbn"));

    let mut external_ids = Vec::new();
    if let Some(isbn) = isbn {
        external_ids.push((
            "isbn".to_string(),
            isbn.to_string(),
            Some(format!("https://www.goodreads.com/search?q={isbn}")),
        ));
    }
    if let Some(book_id) = columns.cell(record, "book id") {
        external_ids.push(("goodreads".to_string(), book_id.to_string(), None));
    }

    let author = columns.cell(record, "author").map(str::to_string);
    let mut people = Vec::new();
    if let Some(author) = author {
        people.push(("author".to_string(), author));
    }

    let release_year = columns
        .cell(record, "year published")
        .or_else(|| columns.cell(record, "original publication year"))
        .map(str::to_string);

    ParsedItem {
        row,
        title: columns
            .cell(record, "title")
            .unwrap_or_default()
            .to_string(),
        title_original: None,
        alt_titles: Vec::new(),
        content_type: Some("book".to_string()),
        format: None,
        pub_status: None,
        start_date: None,
        end_date: None,
        release_year,
        language: None,
        country: None,
        content_rating: None,
        pages: columns.cell(record, "number of pages").map(str::to_string),
        duration_min: None,
        ep_count: None,
        ch_count: None,
        synopsis: None,
        people,
        genres: Vec::new(),
        tags: columns
            .cell(record, "bookshelves")
            .map(|cell| shared::split_values(cell, ", "))
            .unwrap_or_default(),
        external_ids,
        cover_url: None,
        banner_url: None,
        my_status: exclusive_shelf(columns.cell(record, "exclusive shelf")),
        my_rating: columns
            .cell(record, "my rating")
            .and_then(shared::rating_0_5_to_0_10),
        my_review: columns.cell(record, "my review").map(str::to_string),
        progress: None,
        started_at: None,
        completed_at: columns
            .cell(record, "date read")
            .and_then(shared::normalize_date),
        repeat_count: None,
    }
}

/// Goodreads `Exclusive Shelf` → `CoreStatus::as_str()` values.
fn exclusive_shelf(value: Option<&str>) -> Option<String> {
    match value.map(str::trim) {
        Some("read") => Some("completed".to_string()),
        Some("currently-reading") => Some("in_progress".to_string()),
        Some("to-read") => Some("planned".to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOODREADS_CSV: &str = "\u{feff}\"Book Id\",\"Title\",\"Author\",\"ISBN\",\"ISBN13\",\
        \"My Rating\",\"Number of Pages\",\"Year Published\",\"Date Read\",\"Bookshelves\",\
        \"Exclusive Shelf\",\"My Review\"\n\
        1,\"Sword of the Dawn\",\"Jane Doe\",\"0000000000\",\"9780000000001\",\"4\",320,2025,\
        \"2026/01/05\",\"fantasy, classic\",\"read\",\"Lovely.\"\n\
        2,\"Nightfall\",\"Jane Doe\",\"\",\"\",\"0\",\"\",\"\",\"\",\"\",\"to-read\",\"\"\n";

    #[test]
    fn parses_rows_into_items_with_user_state() {
        let items = GoodreadsParser.parse(GOODREADS_CSV).expect("valid csv");
        assert_eq!(items.len(), 2);

        let first = &items[0];
        assert_eq!(first.title, "Sword of the Dawn");
        assert_eq!(first.content_type.as_deref(), Some("book"));
        assert_eq!(first.pages.as_deref(), Some("320"));
        assert_eq!(first.release_year.as_deref(), Some("2025"));
        assert_eq!(
            first.people,
            vec![("author".to_string(), "Jane Doe".to_string())]
        );
        assert_eq!(first.tags, vec!["fantasy", "classic"]);
        assert_eq!(
            first.external_ids,
            vec![
                (
                    "isbn".to_string(),
                    "9780000000001".to_string(),
                    Some("https://www.goodreads.com/search?q=9780000000001".to_string())
                ),
                ("goodreads".to_string(), "1".to_string(), None),
            ],
            "ISBN13 preferred over ISBN, plus the Book Id"
        );
        assert_eq!(first.my_status.as_deref(), Some("completed"));
        assert_eq!(first.my_rating.as_deref(), Some("8"), "4/5 → 8/10");
        assert_eq!(first.completed_at.as_deref(), Some("2026-01-05"));
        assert_eq!(first.my_review.as_deref(), Some("Lovely."));

        let second = &items[1];
        assert_eq!(second.title, "Nightfall");
        assert_eq!(second.my_status.as_deref(), Some("planned"));
        assert_eq!(second.my_rating, None, "rating 0 is treated as unrated");
        assert_eq!(second.completed_at, None);
        assert_eq!(
            second.external_ids,
            vec![("goodreads".to_string(), "2".to_string(), None)],
            "only the Book Id is kept (no isbn)"
        );
    }

    #[test]
    fn isbn_is_skipped_when_blank() {
        let source = "Title,ISBN13\nDune,\n";
        let items = GoodreadsParser.parse(source).expect("csv");
        assert!(items[0].external_ids.is_empty());
    }

    #[test]
    fn tolerates_ragged_rows() {
        let source = "Title,Author,My Rating\nSword\nNightfall,Jane,3\n";
        let items = GoodreadsParser.parse(source).expect("csv");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "Sword");
        assert!(items[0].people.is_empty());
        assert_eq!(items[1].my_rating.as_deref(), Some("6"), "3/5 → 6/10");
    }
}
