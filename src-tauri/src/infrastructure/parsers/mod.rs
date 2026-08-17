//! File-format parsers for the import pipeline (MISSION-068).
//!
//! Both parsers implement `domain::import::ImportParser`, so they plug into the
//! MISSION-067 pipeline unchanged (parse → validate → normalize → dedup →
//! preview → commit). Format docs live in ARCHITECTURE §6:
//!
//!   - `JsonParser` — the **MyLore JSON import format** (a top-level array of
//!     item objects).
//!   - `CsvParser`  — generic CSV with a user-supplied **column mapping**
//!     (`CsvMapping`, built by the MISSION-068 mapping UI).

pub mod csv;
pub mod json;

pub use csv::{csv_headers, CsvMapping, CsvParser};
pub use json::JsonParser;
