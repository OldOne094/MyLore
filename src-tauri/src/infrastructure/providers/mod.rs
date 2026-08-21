//! Provider adapters (MISSION-054+).
//!
//! Each adapter is a thin normalizer: provider HTTP → unified domain types
//! (`domain::provider`) + typed `ProviderError`s, with all policy applied by
//! `application::providers::coordinator`. AniList, TMDB, MangaDex, OpenLibrary,
//! NovelUpdates, Jikan (anime fallback), Google Books (book fallback) and
//! Hardcover (optional third book provider) and Bangumi (CN ACGN) land here.
//! The coordinator fans out to *every* provider that serves a domain, so a
//! primary that fails never fails the search — that's how fallbacks work
//! (MISSION-058).

pub mod anilist;
pub mod bangumi;
pub mod googlebooks;
pub mod hardcover;
pub mod jikan;
pub mod mangadex;
pub mod novelupdates;
pub mod openlibrary;
pub mod tmdb;

pub use anilist::{anilist_config, AniListClient, AniListProvider, PROVIDER_ID};
pub use bangumi::{
    bangumi_config, BangumiClient, BangumiProvider, PROVIDER_ID as BANGUMI_PROVIDER_ID,
};
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
        bangumi::PROVIDER_ID => Ok(Arc::new(BangumiProvider::new(BangumiClient::new()))),
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
        bangumi_config(),
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
        bangumi_config(),
        tmdb_config(),
        mangadex_config(),
        openlibrary_config(),
        novelupdates_config(),
        jikan_config(),
        googlebooks_config(),
        hardcover_config(),
    ]
}

/// Offline-test harness shared by adapter tests (MISSION-098). Every provider
/// test serves recorded fixtures from tests/fixtures/<provider>/ through a
/// wiremock server the client injected base URL points at, so the suite never
/// touches the real network.
#[cfg(test)]
pub(crate) mod test_support {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// Read a recorded fixture under tests/fixtures/<provider>/<name>.
    pub(crate) fn fixture(provider: &str, name: &str) -> String {
        std::fs::read_to_string(format!(
            "{}/tests/fixtures/{provider}/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap_or_else(|error| panic!("{provider}/{name} fixture missing: {error}"))
    }

    /// Mount a recorded fixture: GET route serves 200 + body.
    pub(crate) async fn mount_get(server: &MockServer, route: &str, provider: &str, name: &str) {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(ResponseTemplate::new(200).set_body_string(fixture(provider, name)))
            .mount(server)
            .await;
    }

    /// Mount a recorded fixture: POST route serves 200 + body.
    pub(crate) async fn mount_post(server: &MockServer, route: &str, provider: &str, name: &str) {
        Mock::given(method("POST"))
            .and(path(route))
            .respond_with(ResponseTemplate::new(200).set_body_string(fixture(provider, name)))
            .mount(server)
            .await;
    }

    /// Fixture integrity (MISSION-098): every committed recording must be
    /// non-empty and parse as JSON when it carries the .json extension,
    /// guarding offline CI against truncated or hand-edit-corrupted files.
    #[test]
    fn all_committed_fixtures_are_intact() {
        let root = format!("{}/tests/fixtures", env!("CARGO_MANIFEST_DIR"));
        let mut checked = 0;
        for entry in std::fs::read_dir(&root).expect("fixtures dir") {
            let provider_dir = entry.expect("provider dir").path();
            if !provider_dir.is_dir() || provider_dir.file_name().expect("name") == "import" {
                continue; // import fixtures are CSV/JSON inputs covered elsewhere
            }
            for file in std::fs::read_dir(&provider_dir).expect("provider fixtures") {
                let path = file.expect("fixture file").path();
                let body = std::fs::read_to_string(&path).expect("readable");
                assert!(!body.trim().is_empty(), "{} is empty", path.display());
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    serde_json::from_str::<serde_json::Value>(&body)
                        .unwrap_or_else(|error| panic!("{} invalid JSON: {error}", path.display()));
                }
                checked += 1;
            }
        }
        assert!(checked >= 20, "expected the full corpus, got {checked}");
    }
}
