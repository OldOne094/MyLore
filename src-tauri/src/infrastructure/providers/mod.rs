//! Provider adapters (MISSION-054+).
//!
//! Each adapter is a thin normalizer: provider HTTP → unified domain types
//! (`domain::provider`) + typed `ProviderError`s, with all policy applied by
//! `application::providers::coordinator`. AniList, TMDB and MangaDex land here
//! first; OpenLibrary follows (MISSION-057).

pub mod anilist;
pub mod mangadex;
pub mod tmdb;

pub use anilist::{anilist_config, AniListClient, AniListProvider, PROVIDER_ID};
pub use mangadex::{
    mangadex_config, MangaDexClient, MangaDexProvider, PROVIDER_ID as MANGADEX_PROVIDER_ID,
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
}
