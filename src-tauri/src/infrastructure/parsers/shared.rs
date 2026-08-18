//! Small helpers shared by the profile-CSV parsers (MISSION-072).

use std::collections::HashMap;

use csv::{ReaderBuilder, StringRecord};

use crate::domain::import::ImportError;

/// Read the first record of a CSV file and return its headers lower-cased and
/// trimmed (used both by `detect` and by the profile parsers for a
/// case-insensitive column lookup).
pub(crate) fn sniff_csv_columns(source: &str, delimiter: u8) -> Result<Vec<String>, ImportError> {
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    let mut reader = ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(false)
        .flexible(true)
        .from_reader(source.as_bytes());
    let Some(result) = reader.records().next() else {
        return Ok(Vec::new());
    };
    let header = result
        .map_err(|error| ImportError::Parse(format!("could not read the CSV header: {error}")))?;
    Ok(header
        .iter()
        .map(|name| name.trim().to_ascii_lowercase())
        .collect())
}

/// A column lookup built from lower-cased headers.
pub(crate) struct Columns {
    by_name: HashMap<String, usize>,
}

impl Columns {
    pub(crate) fn from_headers(headers: &[String]) -> Self {
        let mut by_name = HashMap::new();
        for (index, name) in headers.iter().enumerate() {
            if !name.is_empty() {
                by_name.entry(name.clone()).or_insert(index);
            }
        }
        Self { by_name }
    }

    /// Index of the first header equal to `name`, or `None`.
    pub(crate) fn index_of(&self, name: &str) -> Option<usize> {
        self.by_name.get(name).copied()
    }

    pub(crate) fn cell<'a>(&self, record: &'a StringRecord, name: &str) -> Option<&'a str> {
        let index = self.index_of(name)?;
        record
            .get(index)
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }
}

/// Split a multi-value cell on a separator, trimming and dropping empties.
pub(crate) fn split_values(cell: &str, separator: &str) -> Vec<String> {
    cell.split(separator)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

/// A 0–5 rating (Goodreads, StoryGraph) → the app's 0–10 integer scale.
pub(crate) fn rating_0_5_to_0_10(value: &str) -> Option<String> {
    let rating: f64 = value.trim().parse().ok()?;
    if rating <= 0.0 {
        return None;
    }
    let scaled = (rating * 2.0).round().clamp(0.0, 10.0);
    Some(scaled.to_string())
}

/// Accept `YYYY-MM-DD` and `YYYY/MM/DD`, normalizing to `YYYY-MM-DD`.
pub(crate) fn normalize_date(value: &str) -> Option<String> {
    let value = value.trim();
    let mut parts = value.splitn(3, ['-', '/']);
    let year = parts.next()?;
    let month = parts.next()?;
    let day = parts.next()?;
    let (year, month, day) = match (
        year.parse::<i64>(),
        month.parse::<i64>(),
        day.parse::<i64>(),
    ) {
        (Ok(y), Ok(m), Ok(d)) if y > 0 && (1..=12).contains(&m) && (1..=31).contains(&d) => {
            (y, m, d)
        }
        _ => return None,
    };
    Some(format!("{year:04}-{month:02}-{day:02}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniffs_headers_lower_cased() {
        let columns = sniff_csv_columns("Title,Author\nSword,Doe", b',').expect("csv");
        assert_eq!(columns, vec!["title", "author"]);
    }

    #[test]
    fn sniffs_headers_for_tab_delimited() {
        let columns = sniff_csv_columns("Title\tAuthor\nSword\tDoe", b'\t').expect("csv");
        assert_eq!(columns, vec!["title", "author"]);
    }

    #[test]
    fn empty_source_has_no_headers() {
        let columns = sniff_csv_columns("", b',').expect("csv");
        assert!(columns.is_empty());
    }

    #[test]
    fn ratings_scale_to_0_10() {
        assert_eq!(rating_0_5_to_0_10("5"), Some("10".to_string()));
        assert_eq!(rating_0_5_to_0_10("4.5"), Some("9".to_string()));
        assert_eq!(rating_0_5_to_0_10("0"), None);
        assert_eq!(rating_0_5_to_0_10(""), None);
        assert_eq!(rating_0_5_to_0_10("not a number"), None);
    }

    #[test]
    fn dates_normalize_slashes_and_dashes() {
        assert_eq!(normalize_date("2026/01/05"), Some("2026-01-05".to_string()));
        assert_eq!(normalize_date("2026-01-05"), Some("2026-01-05".to_string()));
        assert_eq!(normalize_date("2026/1/5"), Some("2026-01-05".to_string()));
        assert_eq!(normalize_date("2026-13-01"), None);
        assert_eq!(normalize_date("n/a"), None);
    }
}
