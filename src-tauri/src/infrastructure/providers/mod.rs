//! Provider adapters (MISSION-054+).
//!
//! Each adapter is a thin normalizer: provider HTTP → unified domain types
//! (`domain::provider`) + typed `ProviderError`s, with all policy applied by
//! `application::providers::coordinator`. AniList, TMDB, MangaDex, OpenLibrary,
//! NovelUpdates, Jikan (anime fallback) and Google Books (book fallback) land
//! here. The coordinator fans out to *every* provider that serves a domain, so
//! a primary that fails never fails the search — that's how fallbacks work
//! (MISSION-058).

pub mod anilist;
pub mod googlebooks;
pub mod jikan;
pub mod mangadex;
pub mod novelupdates;
pub mod openlibrary;
pub mod tmdb;

pub use anilist::{anilist_config, AniListClient, AniListProvider, PROVIDER_ID};
pub use googlebooks::{
    googlebooks_config, GoogleBooksClient, GoogleBooksProvider,
    PROVIDER_ID as GOOGLEBOOKS_PROVIDER_ID,
};
pub use jikan::{jikan_config, JikanClient, JikanProvider, PROVIDER_ID as JIKAN_PROVIDER_ID};
pub use mangadex::{
    mangadex_config, MangaDexClient, MangaDexProvider, PROVIDER_ID as MANGADEX_PROVIDER_ID,
};
pub use novelupdates::{
    novelupdates_config, NovelUpdatesClient, NovelUpdatesProvider,
    PROVIDER_ID as NOVELUPDATES_PROVIDER_ID,
};
pub use openlibrary::{
    openlibrary_config, OpenLibraryClient, OpenLibraryProvider,
    PROVIDER_ID as OPENLIBRARY_PROVIDER_ID,
};
pub use tmdb::{tmdb_config, TmdbClient, TmdbProvider, PROVIDER_ID as TMDB_PROVIDER_ID};

use std::sync::Arc;

use crate::application::providers::config::ProviderConfig;
use crate::domain::provider::Provider;

/// The provider set registered at app startup. Grows as adapters land. TMDB's
/// API key is injected by the settings UI (MISSION-063) via the OS keyring —
/// the keyless client here only works against mocks until then.
pub fn default_provider_entries() -> Vec<(ProviderConfig, Arc<dyn Provider>)> {
    vec![
        (
            anilist_config(),
            Arc::new(AniListProvider::new(AniListClient::new())) as Arc<dyn Provider>,
        ),
        (
            tmdb_config(),
            Arc::new(TmdbProvider::new(TmdbClient::new())) as Arc<dyn Provider>,
        ),
        (
            mangadex_config(),
            Arc::new(MangaDexProvider::new(MangaDexClient::new())) as Arc<dyn Provider>,
        ),
        (
            openlibrary_config(),
            Arc::new(OpenLibraryProvider::new(OpenLibraryClient::new())) as Arc<dyn Provider>,
        ),
        (
            novelupdates_config(),
            Arc::new(NovelUpdatesProvider::new(NovelUpdatesClient::new()))
                as Arc<dyn Provider>,
        ),
        (
            jikan_config(),
            Arc::new(JikanProvider::new(JikanClient::new())) as Arc<dyn Provider>,
        ),
        (
            googlebooks_config(),
            Arc::new(GoogleBooksProvider::new(GoogleBooksClient::new())) as Arc<dyn Provider>,
        ),
    ]
}

/// Offline-test helpers shared by adapter tests.
#[cfg(test)]
pub(crate) mod test_support {
    /// Read a recorded fixture under `tests/fixtures/<provider>/`.
    pub(crate) fn anilist_fixture(name: &str) -> String {
        std::fs::read_to_string(format!(
            "{}/tests/fixtures/anilist/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .expect("anilist fixture file exists")
    }

    /// Read a recorded fixture under `tests/fixtures/tmdb/`.
    pub(crate) fn tmdb_fixture(name: &str) -> String {
        std::fs::read_to_string(format!(
            "{}/tests/fixtures/tmdb/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .expect("tmdb fixture file exists")
    }

    /// Read a recorded fixture under `tests/fixtures/mangadex/`.
    pub(crate) fn mangadex_fixture(name: &str) -> String {
        std::fs::read_to_string(format!(
            "{}/tests/fixtures/mangadex/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .expect("mangadex fixture file exists")
    }

    /// Read a recorded fixture under `tests/fixtures/openlibrary/`.
    pub(crate) fn openlibrary_fixture(name: &str) -> String {
        std::fs::read_to_string(format!(
            "{}/tests/fixtures/openlibrary/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .expect("openlibrary fixture file exists")
    }

    /// Read a recorded fixture under `tests/fixtures/jikan/`.
    pub(crate) fn jikan_fixture(name: &str) -> String {
        std::fs::read_to_string(format!(
            "{}/tests/fixtures/jikan/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .expect("jikan fixture file exists")
    }

    /// Read a recorded fixture under `tests/fixtures/googlebooks/`.
    pub(crate) fn googlebooks_fixture(name: &str) -> String {
        std::fs::read_to_string(format!(
            "{}/tests/fixtures/googlebooks/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .expect("googlebooks fixture file exists")
    }

    /// Read a fixture under `tests/fixtures/novelupdates/` (hand-built from the
    /// LNReader plugin's selectors — NU's Cloudflare layer blocks recording).
    pub(crate) fn novelupdates_fixture(name: &str) -> String {
        std::fs::read_to_string(format!(
            "{}/tests/fixtures/novelupdates/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .expect("novelupdates fixture file exists")
    }
}
