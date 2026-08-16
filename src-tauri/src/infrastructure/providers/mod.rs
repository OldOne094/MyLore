//! Provider adapters (MISSION-054+).
//!
//! Each adapter is a thin normalizer: provider HTTP → unified domain types
//! (`domain::provider`) + typed `ProviderError`s, with all policy applied by
//! `application::providers::coordinator`. AniList, TMDB, MangaDex, OpenLibrary,
//! NovelUpdates, Jikan (anime fallback), Google Books (book fallback) and
//! Hardcover (optional third book provider) land here. The coordinator fans out
//! to *every* provider that serves a domain, so a primary that fails never
//! fails the search — that's how fallbacks work (MISSION-058).

pub mod anilist;
pub mod googlebooks;
pub mod hardcover;
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
pub use hardcover::{
    hardcover_config, HardcoverClient, HardcoverProvider, PROVIDER_ID as HARDCOVER_PROVIDER_ID,
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
use crate::application::providers::settings::{EntryBuilder, ProviderEntry};
use crate::domain::provider::Provider;

/// Construct one provider adapter by id, injecting its API key when the adapter
/// reads one from its client (TMDB, Google Books). Unknown ids are rejected.
/// The settings service (MISSION-063) rebuilds the coordinator through this so
/// a key change takes effect without a restart — keys are never logged.
pub fn build_adapter(id: &str, api_key: Option<&str>) -> Result<Arc<dyn Provider>, String> {
    match id {
        anilist::PROVIDER_ID => Ok(Arc::new(AniListProvider::new(AniListClient::new()))),
        tmdb::PROVIDER_ID => {
            let mut client = TmdbClient::new();
            if let Some(key) = api_key {
                client = client.with_api_key(key);
            }
            Ok(Arc::new(TmdbProvider::new(client)))
        }
        mangadex::PROVIDER_ID => Ok(Arc::new(MangaDexProvider::new(MangaDexClient::new()))),
        openlibrary::PROVIDER_ID => {
            Ok(Arc::new(OpenLibraryProvider::new(OpenLibraryClient::new())))
        }
        novelupdates::PROVIDER_ID => Ok(Arc::new(NovelUpdatesProvider::new(
            NovelUpdatesClient::new(),
        ))),
        jikan::PROVIDER_ID => Ok(Arc::new(JikanProvider::new(JikanClient::new()))),
        googlebooks::PROVIDER_ID => {
            let mut client = GoogleBooksClient::new();
            if let Some(key) = api_key {
                client = client.with_api_key(key);
            }
            Ok(Arc::new(GoogleBooksProvider::new(client)))
        }
        hardcover::PROVIDER_ID => {
            let mut client = HardcoverClient::new();
            if let Some(key) = api_key {
                client = client.with_api_key(key);
            }
            Ok(Arc::new(HardcoverProvider::new(client)))
        }
        other => Err(format!("unknown provider adapter {other:?}")),
    }
}

/// The production `EntryBuilder`: rebuilds every `(config, adapter)` pair from
/// configs via `build_adapter`. Injectable so settings tests can substitute
/// fake adapters without touching the network.
pub struct StdEntryBuilder;

impl EntryBuilder for StdEntryBuilder {
    fn build(&self, configs: &[ProviderConfig]) -> Result<Vec<ProviderEntry>, String> {
        configs
            .iter()
            .map(|config| {
                Ok((
                    config.clone(),
                    build_adapter(&config.id, config.api_key.as_deref())?,
                ))
            })
            .collect()
    }
}

/// The provider set registered at app startup. Grows as adapters land. TMDB's
/// API key is injected by the settings UI (MISSION-063) via the OS keyring —
/// the keyless client here only works against mocks until then.
pub fn default_provider_entries() -> Vec<ProviderEntry> {
    [
        anilist_config(),
        tmdb_config(),
        mangadex_config(),
        openlibrary_config(),
        novelupdates_config(),
        jikan_config(),
        googlebooks_config(),
        hardcover_config(),
    ]
    .into_iter()
    .map(|config| {
        let adapter = build_adapter(&config.id, config.api_key.as_deref()).expect("known adapter");
        (config, adapter)
    })
    .collect()
}

/// The default per-provider configs, in registration order (MISSION-063). The
/// settings service reads persisted enabled flags and keyring keys on top of
/// these, then rebuilds the coordinator.
pub fn default_provider_configs() -> Vec<ProviderConfig> {
    vec![
        anilist_config(),
        tmdb_config(),
        mangadex_config(),
        openlibrary_config(),
        novelupdates_config(),
        jikan_config(),
        googlebooks_config(),
        hardcover_config(),
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

    /// Read a fixture under `tests/fixtures/hardcover/` (hand-built from the
    /// published GraphQL schema + Typesense search docs).
    pub(crate) fn hardcover_fixture(name: &str) -> String {
        std::fs::read_to_string(format!(
            "{}/tests/fixtures/hardcover/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .expect("hardcover fixture file exists")
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
