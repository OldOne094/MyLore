//! CSV import with a user-supplied column mapping (MISSION-068, ARCHITECTURE
//! §6).
//!
//! A `CsvMapping` names the CSV column for each import field (a field with no
//! column is left unmapped → `None` → the pipeline's validator/normalizer
//! decide how to degrade the row). Multi-value fields (`alt_titles`, `genres`,
//! `tags`, `external_id`) split their cell on `separator` (default `,`).
//! `default_content_type` lets a file without a type column still import —
//! every row gets that type (validation requires `content_type`).
//!
//! Structural file errors abort with `ImportError::Parse`; row problems flow
//! to the per-row validator. Files are parsed with `flexible(true)`, so a
//! ragged trailing line never aborts the batch.

use std::collections::HashMap;

use csv::{ReaderBuilder, StringRecord};

use crate::domain::import::{ImportError, ImportParser, ParsedItem};

/// Which CSV column feeds each import field (built by the mapping UI).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CsvMapping {
    pub title: Option<String>,
    pub title_original: Option<String>,
    pub alt_titles: Option<String>,
    pub content_type: Option<String>,
    pub default_content_type: Option<String>,
    pub format: Option<String>,
    pub pub_status: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub release_year: Option<String>,
    pub language: Option<String>,
    pub country: Option<String>,
    pub content_rating: Option<String>,
    pub pages: Option<String>,
    pub duration_min: Option<String>,
    pub ep_count: Option<String>,
    pub ch_count: Option<String>,
    pub synopsis: Option<String>,
    pub author: Option<String>,
    pub artist: Option<String>,
    pub studio: Option<String>,
    pub genres: Option<String>,
    pub tags: Option<String>,
    pub external_id: Option<String>,
    pub cover_url: Option<String>,
    pub banner_url: Option<String>,
    /// CSV field delimiter (default `,`; `\\t` means a tab).
    pub delimiter: String,
    /// List separator for multi-value cells (default `,`).
    pub separator: String,
}

impl Default for CsvMapping {
    fn default() -> Self {
        Self {
            title: None,
            title_original: None,
            alt_titles: None,
            content_type: None,
            default_content_type: None,
            format: None,
            pub_status: None,
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
            author: None,
            artist: None,
            studio: None,
            genres: None,
            tags: None,
            external_id: None,
            cover_url: None,
            banner_url: None,
            delimiter: ",".to_string(),
            separator: ",".to_string(),
        }
    }
}

/// Parses CSV files into pipeline items using a column mapping.
pub struct CsvParser {
    mapping: CsvMapping,
}

impl CsvParser {
    pub fn new(mapping: CsvMapping) -> Self {
        Self { mapping }
    }

    fn to_item(&self, row: usize, record: &StringRecord, columns: &ColumnIndex) -> ParsedItem {
        let mut people = Vec::new();
        if let Some(name) = self.cell(record, columns, &self.mapping.author) {
            people.push(("author".to_string(), name));
        }
        if let Some(name) = self.cell(record, columns, &self.mapping.artist) {
            people.push(("artist".to_string(), name));
        }
        if let Some(name) = self.cell(record, columns, &self.mapping.studio) {
            people.push(("studio".to_string(), name));
        }

        let content_type = match self
            .mapping
            .default_content_type
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(value) => Some(value.to_string()),
            None => self.cell(record, columns, &self.mapping.content_type),
        };

        ParsedItem {
            row,
            title: self
                .cell(record, columns, &self.mapping.title)
                .unwrap_or_default(),
            title_original: self.cell(record, columns, &self.mapping.title_original),
            alt_titles: self.split_cell(self.cell(record, columns, &self.mapping.alt_titles)),
            content_type,
            format: self.cell(record, columns, &self.mapping.format),
            pub_status: self.cell(record, columns, &self.mapping.pub_status),
            start_date: self.cell(record, columns, &self.mapping.start_date),
            end_date: self.cell(record, columns, &self.mapping.end_date),
            release_year: self.cell(record, columns, &self.mapping.release_year),
            language: self.cell(record, columns, &self.mapping.language),
            country: self.cell(record, columns, &self.mapping.country),
            content_rating: self.cell(record, columns, &self.mapping.content_rating),
            pages: self.cell(record, columns, &self.mapping.pages),
            duration_min: self.cell(record, columns, &self.mapping.duration_min),
            ep_count: self.cell(record, columns, &self.mapping.ep_count),
            ch_count: self.cell(record, columns, &self.mapping.ch_count),
            synopsis: self.cell(record, columns, &self.mapping.synopsis),
            people,
            genres: self.split_cell(self.cell(record, columns, &self.mapping.genres)),
            tags: self.split_cell(self.cell(record, columns, &self.mapping.tags)),
            external_ids: self.external_ids(record, columns),
            cover_url: self.cell(record, columns, &self.mapping.cover_url),
            banner_url: self.cell(record, columns, &self.mapping.banner_url),
        }
    }

    fn cell(
        &self,
        record: &StringRecord,
        columns: &ColumnIndex,
        column: &Option<String>,
    ) -> Option<String> {
        let index = columns.index_of(column)?;
        record
            .get(index)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    }

    fn split_cell(&self, cell: Option<String>) -> Vec<String> {
        let Some(cell) = cell else {
            return Vec::new();
        };
        if self.mapping.separator.is_empty() {
            return vec![cell];
        }
        cell.split(&self.mapping.separator)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .collect()
    }

    fn external_ids(
        &self,
        record: &StringRecord,
        columns: &ColumnIndex,
    ) -> Vec<(String, String, Option<String>)> {
        let Some(cell) = self.cell(record, columns, &self.mapping.external_id) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for part in self.split_cell(Some(cell)) {
            if let Some((provider, value)) = part.split_once(':') {
                let provider = provider.trim();
                let value = value.trim();
                if !provider.is_empty() && !value.is_empty() {
                    out.push((provider.to_string(), value.to_string(), None));
                }
            }
        }
        out
    }
}

impl ImportParser for CsvParser {
    fn parse(&self, source: &str) -> Result<Vec<ParsedItem>, ImportError> {
        let delimiter = parse_delimiter(&self.mapping.delimiter)?;
        let source = source.strip_prefix('\u{feff}').unwrap_or(source);
        let mut reader = ReaderBuilder::new()
            .delimiter(delimiter)
            .has_headers(true)
            .flexible(true)
            .from_reader(source.as_bytes());
        let header = reader
            .headers()
            .map_err(|error| ImportError::Parse(format!("missing CSV header row: {error}")))?;
        let columns = ColumnIndex::from_headers(header);
        let mut items = Vec::new();
        for (index, result) in reader.records().enumerate() {
            let record = result.map_err(|error| {
                ImportError::Parse(format!("CSV record {}: {error}", index + 2))
            })?;
            items.push(self.to_item(index + 1, &record, &columns));
        }
        Ok(items)
    }
}

/// Read just the header row of a CSV (for the mapping UI's column pickers).
pub fn csv_headers(source: &str, delimiter: &str) -> Result<Vec<String>, ImportError> {
    let delimiter = parse_delimiter(delimiter)?;
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    let mut reader = ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(false)
        .flexible(true)
        .from_reader(source.as_bytes());
    let Some(result) = reader.records().next() else {
        return Ok(Vec::new());
    };
    let header =
        result.map_err(|error| ImportError::Parse(format!("invalid CSV header row: {error}")))?;
    Ok(header.iter().map(str::trim).map(str::to_string).collect())
}

fn parse_delimiter(raw: &str) -> Result<u8, ImportError> {
    let value = if raw == "\\t" { "\t" } else { raw };
    match value.as_bytes() {
        [byte] => Ok(*byte),
        _ => Err(ImportError::Parse(
            "CSV delimiter must be a single character".to_string(),
        )),
    }
}

/// Header name → record index lookup.
struct ColumnIndex {
    by_name: HashMap<String, usize>,
}

impl ColumnIndex {
    fn from_headers(header: &StringRecord) -> Self {
        let mut by_name = HashMap::new();
        for (index, name) in header.iter().enumerate() {
            let name = name.trim();
            if !name.is_empty() {
                by_name.entry(name.to_string()).or_insert(index);
            }
        }
        Self { by_name }
    }

    fn index_of(&self, column: &Option<String>) -> Option<usize> {
        let name = column.as_deref()?.trim();
        self.by_name.get(name).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mapping() -> CsvMapping {
        CsvMapping {
            title: Some("Title".to_string()),
            author: Some("Author".to_string()),
            genres: Some("Genres".to_string()),
            pages: Some("Pages".to_string()),
            external_id: Some("AniList ID".to_string()),
            ..CsvMapping::default()
        }
    }

    #[test]
    fn parses_mapped_columns_into_items() {
        let source = "Title,Author,Genres,Pages,AniList ID\n\
                      Sword of the Dawn,Test Author,\"Fantasy, Adventure\",320,anilist:42\n\
                      Berserk,Kentaro Miura,Seinen,380,mal:5";
        let items = CsvParser::new(mapping()).parse(source).expect("valid");

        assert_eq!(items.len(), 2);
        let first = &items[0];
        assert_eq!(first.row, 1);
        assert_eq!(first.title, "Sword of the Dawn");
        assert_eq!(
            first.people,
            vec![("author".to_string(), "Test Author".to_string())]
        );
        assert_eq!(first.genres, vec!["Fantasy", "Adventure"]);
        assert_eq!(first.pages.as_deref(), Some("320"));
        assert_eq!(
            first.external_ids,
            vec![("anilist".to_string(), "42".to_string(), None)]
        );

        let second = &items[1];
        assert_eq!(second.row, 2);
        assert_eq!(second.title, "Berserk");
        assert_eq!(
            second.external_ids,
            vec![("mal".to_string(), "5".to_string(), None)]
        );
    }

    #[test]
    fn row_numbers_skip_the_header() {
        let source = "Title\nAlpha\nBeta\nGamma";
        let items = CsvParser::new(mapping()).parse(source).expect("valid");
        let rows: Vec<usize> = items.iter().map(|item| item.row).collect();
        assert_eq!(rows, vec![1, 2, 3]);
    }

    #[test]
    fn unmapped_fields_stay_none() {
        let source = "Title\nAlpha";
        let items = CsvParser::new(mapping()).parse(source).expect("valid");
        let item = &items[0];
        assert_eq!(item.title, "Alpha");
        assert_eq!(item.content_type, None);
        assert_eq!(item.release_year, None);
        assert!(item.genres.is_empty());
    }

    #[test]
    fn default_content_type_ignores_the_column() {
        let mut map = mapping();
        map.default_content_type = Some("novel".to_string());
        let source = "Title\nAlpha";
        let item = &CsvParser::new(map).parse(source).expect("valid")[0];
        assert_eq!(item.content_type.as_deref(), Some("novel"));
    }

    #[test]
    fn custom_list_separator_splits_cells() {
        let mut map = mapping();
        map.separator = "|".to_string();
        let source = "Title,Genres\nSword,Fantasy|Adventure";
        let item = &CsvParser::new(map).parse(source).expect("valid")[0];
        assert_eq!(item.genres, vec!["Fantasy", "Adventure"]);
    }

    #[test]
    fn tab_delimited_files_parse_with_the_tab_delimiter() {
        let mut map = mapping();
        map.delimiter = "\\t".to_string();
        let source = "Title\tPages\nSword\t320";
        let item = &CsvParser::new(map).parse(source).expect("valid")[0];
        assert_eq!(item.title, "Sword");
        assert_eq!(item.pages.as_deref(), Some("320"));
    }

    #[test]
    fn unknown_mapped_column_is_treated_as_unmapped() {
        let mut map = mapping();
        map.pages = Some("Nope".to_string());
        let source = "Title\nSword";
        let item = &CsvParser::new(map).parse(source).expect("valid")[0];
        assert_eq!(item.title, "Sword");
        assert_eq!(item.pages, None);
    }

    #[test]
    fn ragged_rows_do_not_abort_the_batch() {
        let source = "Title,Author\nAlpha,Jane\nBeta";
        let items = CsvParser::new(mapping()).parse(source).expect("valid");
        assert_eq!(items.len(), 2);
        assert_eq!(
            items[1].people.len(),
            0,
            "missing cell → unmapped, no error"
        );
    }

    #[test]
    fn a_utf8_bom_is_stripped() {
        let source = "\u{feff}Title\nAlpha";
        let item = &CsvParser::new(mapping()).parse(source).expect("valid")[0];
        assert_eq!(item.title, "Alpha");
    }

    #[test]
    fn header_only_file_parses_to_zero_items() {
        let source = "Title,Author";
        assert!(CsvParser::new(mapping())
            .parse(source)
            .expect("valid")
            .is_empty());
    }

    #[test]
    fn blank_title_surfaces_for_validation() {
        let source = "Title\n   ";
        let item = &CsvParser::new(mapping()).parse(source).expect("valid")[0];
        assert_eq!(item.title, "");
    }

    #[test]
    fn external_ids_without_a_provider_colon_are_dropped() {
        let source = "Title,AniList ID\nSword,42\nBeta,mal:5";
        let items = CsvParser::new(mapping()).parse(source).expect("valid");
        assert!(items[0].external_ids.is_empty());
        assert_eq!(
            items[1].external_ids,
            vec![("mal".to_string(), "5".to_string(), None)]
        );
    }

    #[test]
    fn invalid_delimiter_is_a_parse_error() {
        let mut map = mapping();
        map.delimiter = "::".to_string();
        let error = CsvParser::new(map)
            .parse("Title\nAlpha")
            .expect_err("multi-char");
        assert!(matches!(error, ImportError::Parse(_)));
    }

    #[test]
    fn headers_returns_trimmed_column_names() {
        assert_eq!(
            csv_headers(" Title , Author \nAlpha,Jane", ",").expect("headers"),
            vec!["Title", "Author"]
        );
        assert!(csv_headers("", ",").expect("empty").is_empty());
    }
}
