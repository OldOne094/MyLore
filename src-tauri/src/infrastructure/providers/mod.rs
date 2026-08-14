//! Provider adapters (MISSION-054+).
//!
//! Each adapter is a thin normalizer: provider HTTP → unified domain types
//! (`domain::provider`) + typed `ProviderError`s, with all policy applied by
//! `application::providers::coordinator`. AniList lands here first; TMDB,
//! MangaDex and OpenLibrary follow (MISSION-055…057).

pub mod anilist;

pub use anilist::{anilist_config, AniListClient, AniListProvider, PROVIDER_ID};

use std::sync::Arc;

use crate::application::providers::config::ProviderConfig;
use crate::domain::provider::Provider;

/// The provider set registered at app startup. Grows as adapters land.
pub fn default_provider_entries() -> Vec<(ProviderConfig, Arc<dyn Provider>)> {
    vec![(
        anilist_config(),
        Arc::new(AniListProvider::new(AniListClient::new())) as Arc<dyn Provider>,
    )]
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
}
