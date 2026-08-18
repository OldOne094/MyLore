//! File-format parsers for the import pipeline (MISSION-068, MISSION-072).
//!
//! All parsers implement `domain::import::ImportParser`, so they plug into the
//! MISSION-067 pipeline unchanged (parse → validate → normalize → dedup →
//! preview → commit). Format docs live in ARCHITECTURE §6:
//!
//!   - `JsonParser` — the **MyLore JSON import format** (a top-level array of
//!     item objects).
//!   - `CsvParser`  — generic CSV with a user-supplied **column mapping**
//!     (`CsvMapping`, built by the MISSION-068 mapping UI).
//!   - `AniListParser` — the **AniList user export** JSON (MISSION-072):
//!     `mediaListCollection.lists[].entries[]`, each entry carrying the media
//!     plus the user's status / score / progress / dates.
//!   - `GoodreadsParser` / `StorygraphParser` — the **Goodreads** and
//!     **StoryGraph** CSV exports (MISSION-072): fixed, well-known columns with
//!     a built-in mapping (no mapping UI needed).

pub mod anilist;
pub mod csv;
pub mod goodreads;
pub mod json;
pub mod shared;
pub mod storygraph;

pub use anilist::AniListParser;
pub use csv::{csv_headers, CsvMapping, CsvParser};
pub use goodreads::GoodreadsParser;
pub use json::JsonParser;
pub use storygraph::StorygraphParser;
