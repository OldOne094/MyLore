//! The **StoryGraph CSV export** parser (MISSION-072, ARCHITECTURE §6).
//!
//! The StoryGraph `Your Reading Activity` → export produces a CSV with a fixed,
//! well-known header (Title, Author, Reading Status, Start Date, End Date,
//! Rating, Genre, Page Count, Page Number Read, Notes, Book Type, Format, …).
//! The parser reads it with a built-in column mapping — no mapping UI — and
//! feeds the user's list state into the pipeline:
//!
//!   - `Reading Status` (Read / Currently Reading / To Read / Did Not Finish)
//!     → `my_status`
//!   - `Rating` (0–5) → `my_rating` on the 0–10 scale
//!   - `Notes` → `my_review`; `Start Date` / `End Date` → `started_at` /
//!     `completed_at`; `Page Number Read` → `progress`
//!   - `Genre` → `tags`; `Page Count` → `pages`; `Book Type` → `format`
//!
//! Column lookups are case-insensitive. A file that is not a StoryGraph export
//! still parses; its rows lack title/user state and are caught by the
//! pipeline's validator. Structural CSV errors abort with `ImportError::Parse`.

use csv::{ReaderBuilder, StringRecord};

use crate::domain::import::{ImportError, ImportParser, ParsedItem};
use crate::infrastructure::parsers::shared::{self, Columns};

/// Parses the StoryGraph CSV export into pipeline items.
pub struct StorygraphParser;

impl ImportParser for StorygraphParser {
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
                ImportError::Parse(format!("could not read a StoryGraph row: {error}"))
            })?;
            items.push(to_item(items.len() + 1, &record, &columns));
        }
        Ok(items)
    }
}

fn to_item(row: usize, record: &StringRecord, columns: &Columns) -> ParsedItem {
    let author = columns
        .cell(record, "author")
        .or_else(|| columns.cell(record, "contributor"));
    let mut people = Vec::new();
    if let Some(author) = author {
        people.push(("author".to_string(), author.to_string()));
    }

    let mut tags = Vec::new();
    for name in ["genre", "genres"] {
        if let Some(cell) = columns.cell(record, name) {
            tags.extend(shared::split_values(cell, ", "));
        }
    }
    tags.dedup();

    ParsedItem {
        row,
        title: columns
            .cell(record, "title")
            .unwrap_or_default()
            .to_string(),
        title_original: None,
        alt_titles: Vec::new(),
        content_type: Some("book".to_string()),
        format: columns.cell(record, "book type").map(str::to_string),
        pub_status: None,
        start_date: None,
        end_date: None,
        release_year: None,
        language: None,
        country: None,
        content_rating: None,
        pages: columns.cell(record, "page count").map(str::to_string),
        duration_min: None,
        ep_count: None,
        ch_count: None,
        synopsis: None,
        people,
        genres: Vec::new(),
        tags,
        external_ids: Vec::new(),
        cover_url: None,
        banner_url: None,
        my_status: reading_status(columns.cell(record, "reading status")),
        my_rating: columns
            .cell(record, "rating")
            .and_then(shared::rating_0_5_to_0_10),
        my_review: columns.cell(record, "notes").map(str::to_string),
        progress: columns.cell(record, "page number read").map(str::to_string),
        started_at: columns
            .cell(record, "start date")
            .and_then(shared::normalize_date),
        completed_at: columns
            .cell(record, "end date")
            .and_then(shared::normalize_date),
        repeat_count: None,
    }
}

/// StoryGraph `Reading Status` → `CoreStatus::as_str()` values.
fn reading_status(value: Option<&str>) -> Option<String> {
    match value.map(str::trim) {
        Some("Read") => Some("completed".to_string()),
        Some("Currently Reading") => Some("in_progress".to_string()),
        Some("To Read") => Some("planned".to_string()),
        Some("Did Not Finish") => Some("dropped".to_string()),
        Some("Paused") => Some("on_hold".to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STORYGRAPH_CSV: &str = "\"Title\",\"Author\",\"Reading Status\",\"Start Date\",\
        \"End Date\",\"Rating\",\"Genre\",\"Page Count\",\"Page Number Read\",\"Notes\"\n\
        \"Sword of the Dawn\",\"Jane Doe\",\"Read\",\"2026-01-01\",\"2026-02-01\",\"4.5\",\
        \"Fantasy, Adventure\",\"320\",\"320\",\"Lovely.\"\n\
        \"Nightfall\",\"Jane Doe\",\"Currently Reading\",\"2026-03-01\",\"\",\"\",\
        \"\",\"400\",\"120\",\"\"\n";

    #[test]
    fn parses_rows_into_items_with_user_state() {
        let items = StorygraphParser.parse(STORYGRAPH_CSV).expect("valid csv");
        assert_eq!(items.len(), 2);

        let first = &items[0];
        assert_eq!(first.title, "Sword of the Dawn");
        assert_eq!(first.content_type.as_deref(), Some("book"));
        assert_eq!(first.pages.as_deref(), Some("320"));
        assert_eq!(
            first.people,
            vec![("author".to_string(), "Jane Doe".to_string())]
        );
        assert_eq!(first.tags, vec!["Fantasy", "Adventure"]);
        assert_eq!(first.my_status.as_deref(), Some("completed"));
        assert_eq!(first.my_rating.as_deref(), Some("9"), "4.5/5 → 9/10");
        assert_eq!(first.started_at.as_deref(), Some("2026-01-01"));
        assert_eq!(first.completed_at.as_deref(), Some("2026-02-01"));
        assert_eq!(first.progress.as_deref(), Some("320"));
        assert_eq!(first.my_review.as_deref(), Some("Lovely."));

        let second = &items[1];
        assert_eq!(second.title, "Nightfall");
        assert_eq!(second.my_status.as_deref(), Some("in_progress"));
        assert_eq!(second.my_rating, None);
        assert_eq!(second.progress.as_deref(), Some("120"));
        assert_eq!(second.completed_at, None);
    }

    #[test]
    fn did_not_finish_and_paused_map_to_terminal_and_hold() {
        let source = "Title,Reading Status\nA,Did Not Finish\nB,Paused\nC,To Read\n";
        let items = StorygraphParser.parse(source).expect("csv");
        assert_eq!(items[0].my_status.as_deref(), Some("dropped"));
        assert_eq!(items[1].my_status.as_deref(), Some("on_hold"));
        assert_eq!(items[2].my_status.as_deref(), Some("planned"));
    }

    #[test]
    fn tolerates_ragged_rows() {
        let source = "Title,Reading Status\nSword\nNightfall,To Read\n";
        let items = StorygraphParser.parse(source).expect("csv");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "Sword");
        assert_eq!(items[0].my_status, None);
        assert_eq!(items[1].my_status.as_deref(), Some("planned"));
    }
}
