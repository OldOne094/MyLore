//! Provider settings application service (MISSION-063).
//!
//! Owns the live provider set: `enabled` flags (persisted to a small JSON file
//! in the app data dir) and API keys (kept in the OS keyring — never in the
//! JSON file, the webview, or logs). Any change rebuilds the coordinator so it
//! takes effect without a restart. Adapter construction is delegated to an
//! injectable `EntryBuilder` (production: `StdEntryBuilder`, tests: fakes).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::RwLock;

use crate::application::providers::coordinator::{ProviderCoordinator, ProviderInfo};
use crate::application::providers::config::ProviderConfig;
use crate::domain::provider::capabilities::AuthKind;
use crate::domain::provider::Provider;
use crate::error::AppError;
use crate::infrastructure::keyring::SecretStore;

/// One `(config, adapter)` pair for a coordinator rebuild (keeps the builder
/// signature readable).
pub type ProviderEntry = (ProviderConfig, Arc<dyn Provider>);

/// Rebuilds the coordinator's `(config, adapter)` pairs from configs. The
/// settings service calls this on every change; production uses
/// `infrastructure::providers::StdEntryBuilder`, tests inject fakes so they
/// never touch the network or the OS keyring.
pub trait EntryBuilder: Send + Sync {
    fn build(&self, configs: &[ProviderConfig]) -> Result<Vec<ProviderEntry>, String>;
}

/// Read-only provider row for the settings UI.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProviderSettingsView {
    /// Provider id (e.g. `tmdb`).
    pub provider: String,
    /// Human-readable provider name.
    pub name: String,
    /// Whether routing includes this provider.
    pub enabled: bool,
    /// Whether the provider authenticates with a static API key.
    pub requires_key: bool,
    /// Whether a key is currently stored (never the key itself).
    pub has_key: bool,
}

/// Result of a `test_connection` ping.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProviderTestView {
    /// Whether the ping completed (any search results, even zero).
    pub ok: bool,
    /// User-safe failure detail when `ok` is false (empty otherwise).
    pub message: String,
    /// Hits returned by the probe query (informative when `ok`).
    pub results: usize,
}

/// What gets persisted on disk (enabled flags only — never keys).
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct PersistedSettings {
    enabled: HashMap<String, bool>,
}

struct SettingsState {
    configs: Vec<ProviderConfig>,
    coordinator: Arc<ProviderCoordinator>,
}

/// The live, mutable provider configuration. Commands that need the coordinator
/// take `Arc<ProviderSettingsService>` and snapshot `coordinator()` briefly, so
/// settings changes (a coordinator swap) are always observed.
pub struct ProviderSettingsService {
    settings_file: PathBuf,
    store: Box<dyn SecretStore>,
    builder: Arc<dyn EntryBuilder>,
    state: RwLock<SettingsState>,
}

impl ProviderSettingsService {
    /// Build the service from the default configs, layering persisted enabled
    /// flags and keyring keys on top, then building the initial coordinator.
    pub fn load(
        defaults: Vec<ProviderConfig>,
        settings_file: PathBuf,
        store: Box<dyn SecretStore>,
        builder: Arc<dyn EntryBuilder>,
    ) -> Result<Self, AppError> {
        let enabled = load_enabled(&settings_file);
        let mut configs = defaults;
        for config in configs.iter_mut() {
            if let Some(flag) = enabled.get(&config.id) {
                config.enabled = *flag;
            }
            config.api_key = store.get(&config.id);
        }
        let coordinator = rebuild(&builder, &configs)?;
        Ok(Self {
            settings_file,
            store,
            builder,
            state: RwLock::new(SettingsState { configs, coordinator }),
        })
    }

    /// The current coordinator. Callers clone the Arc and release the lock
    /// immediately — a settings write may swap it at any time.
    pub fn coordinator(&self) -> Arc<ProviderCoordinator> {
        self.state.read().unwrap().coordinator.clone()
    }

    /// Snapshot of every registered provider for the settings UI, in
    /// registration order.
    pub fn list(&self) -> Vec<ProviderSettingsView> {
        let state = self.state.read().unwrap();
        state
            .coordinator
            .providers()
            .into_iter()
            .map(|info| {
                let has_key = state
                    .configs
                    .iter()
                    .find(|c| c.id == info.id)
                    .and_then(|c| c.api_key.as_ref())
                    .is_some();
                to_view(info, has_key)
            })
            .collect()
    }

    /// Toggle one provider on/off. Persists the flag and rebuilds the
    /// coordinator so routing honors it immediately.
    pub fn set_enabled(&self, provider: &str, enabled: bool) -> Result<ProviderSettingsView, AppError> {
        {
            let mut state = self.state.write().unwrap();
            let config = state
                .configs
                .iter_mut()
                .find(|c| c.id == provider)
                .ok_or_else(|| AppError::Config(format!("unknown provider {provider:?}")))?;
            if config.enabled != enabled {
                config.enabled = enabled;
                state.coordinator = rebuild(&self.builder, &state.configs)?;
            }
        }
        self.persist_enabled()?;
        self.view(provider)
    }

    /// Store or replace a provider's API key in the OS keyring (blank/whitespace
    /// clears it). Only providers that declare `AuthKind::Key` accept a key.
    /// Rebuilds the coordinator so the new key is used immediately. The key is
    /// never persisted in the JSON settings file and never returned.
    pub fn set_key(&self, provider: &str, api_key: &str) -> Result<ProviderSettingsView, AppError> {
        let trimmed = api_key.trim();
        let (exists, requires_key) = {
            let state = self.state.read().unwrap();
            (
                state.configs.iter().any(|c| c.id == provider),
                state
                    .coordinator
                    .providers()
                    .iter()
                    .any(|p| p.id == provider && p.capabilities.auth == AuthKind::Key),
            )
        };
        if !exists {
            return Err(AppError::Config(format!("unknown provider {provider:?}")));
        }
        if !requires_key {
            return Err(AppError::validation(format!(
                "{provider:?} does not use an API key"
            )));
        }

        {
            let mut state = self.state.write().unwrap();
            let config = state
                .configs
                .iter_mut()
                .find(|c| c.id == provider)
                .ok_or_else(|| AppError::Config(format!("unknown provider {provider:?}")))?;

            if trimmed.is_empty() {
                self.store.delete(provider).map_err(AppError::internal)?;
                config.api_key = None;
            } else {
                self.store.set(provider, trimmed).map_err(AppError::internal)?;
                config.api_key = Some(trimmed.to_string());
            }
            state.coordinator = rebuild(&self.builder, &state.configs)?;
        }
        self.view(provider)
    }

    /// Ping one provider with a probe search, reporting a user-safe outcome.
    /// Runs even when the provider is disabled so a key can be verified before
    /// enabling. Never raises: connection problems are reported in the view.
    pub async fn test_connection(&self, provider: &str) -> Result<ProviderTestView, AppError> {
        let coordinator = self.coordinator();
        let token = coordinator.token();
        match coordinator
            .search_provider(provider, "MyLore", None, &token)
            .await
        {
            Ok(hits) => Ok(ProviderTestView {
                ok: true,
                message: String::new(),
                results: hits.len(),
            }),
            Err(error) => Ok(ProviderTestView {
                ok: false,
                message: error.to_string(),
                results: 0,
            }),
        }
    }

    /// The settings view for one provider.
    pub fn view(&self, provider: &str) -> Result<ProviderSettingsView, AppError> {
        self.list()
            .into_iter()
            .find(|view| view.provider == provider)
            .ok_or_else(|| AppError::Config(format!("unknown provider {provider:?}")))
    }

    /// Persist the enabled flags. Keys never land here.
    fn persist_enabled(&self) -> Result<(), AppError> {
        let enabled = {
            let state = self.state.read().unwrap();
            state
                .configs
                .iter()
                .map(|c| (c.id.clone(), c.enabled))
                .collect::<HashMap<_, _>>()
        };
        let payload = serde_json::to_vec_pretty(&PersistedSettings { enabled })?;
        atomic_write(&self.settings_file, &payload)?;
        Ok(())
    }
}

/// One settings row from coordinator info + keyring presence.
fn to_view(info: ProviderInfo, has_key: bool) -> ProviderSettingsView {
    ProviderSettingsView {
        provider: info.id,
        name: info.name,
        enabled: info.enabled,
        requires_key: info.capabilities.auth == AuthKind::Key,
        has_key,
    }
}

/// Build the coordinator from configs through the injected builder.
fn rebuild(
    builder: &Arc<dyn EntryBuilder>,
    configs: &[ProviderConfig],
) -> Result<Arc<ProviderCoordinator>, AppError> {
    let pairs = builder.build(configs).map_err(AppError::internal)?;
    ProviderCoordinator::new(pairs)
        .map(Arc::new)
        .map_err(AppError::internal)
}

/// Read the persisted enabled flags (missing/corrupt file = all defaults).
fn load_enabled(path: &Path) -> HashMap<String, bool> {
    let Ok(contents) = std::fs::read_to_string(path) else {
        return HashMap::new();
    };
    serde_json::from_str::<PersistedSettings>(&contents)
        .map(|settings| settings.enabled)
        .unwrap_or_default()
}

/// Write bytes atomically (temp file + rename) so a crash never leaves a
/// half-written settings file.
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), std::io::Error> {
    let mut temp = path.as_os_str().to_os_string();
    temp.push(".tmp");
    std::fs::write(&temp, bytes)?;
    std::fs::rename(temp, path)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::application::providers::config::ProviderConfig;
    use crate::domain::enums::ContentType;
    use crate::domain::provider::capabilities::ProviderCapabilities;
    use crate::domain::provider::error::ProviderError;
    use crate::domain::provider::types::ProviderCandidate;
    use crate::infrastructure::keyring::InMemoryKeyring;
    use crate::infrastructure::test_support::cleanup_files;

    fn tmdb_config() -> ProviderConfig {
        ProviderConfig::new("tmdb").with_requests_per_sec(0.0)
    }

    fn openlibrary_config() -> ProviderConfig {
        ProviderConfig::new("openlibrary").with_requests_per_sec(0.0)
    }

    fn defaults() -> Vec<ProviderConfig> {
        vec![tmdb_config(), openlibrary_config()]
    }

    #[derive(Clone)]
    enum Behavior {
        Ok(usize),
        Fail(ProviderError),
    }

    struct FakeProvider {
        id: String,
        behavior: Mutex<Behavior>,
    }

    #[async_trait::async_trait]
    impl Provider for FakeProvider {
        fn id(&self) -> String {
            self.id.clone()
        }
        fn name(&self) -> &str {
            match self.id.as_str() {
                "tmdb" => "TMDB",
                "openlibrary" => "Open Library",
                other => other,
            }
        }
        fn capabilities(&self) -> &ProviderCapabilities {
            const CAPS: ProviderCapabilities = ProviderCapabilities {
                search: true,
                details: false,
                nodes: false,
                related: false,
                reviews: false,
                images: false,
                seasonal: false,
                auth: AuthKind::Key,
            };
            static OPEN_CAPS: ProviderCapabilities = ProviderCapabilities {
                search: true,
                details: false,
                nodes: false,
                related: false,
                reviews: false,
                images: false,
                seasonal: false,
                auth: AuthKind::None,
            };
            match self.id.as_str() {
                "tmdb" => &CAPS,
                _ => &OPEN_CAPS,
            }
        }
        async fn search(
            &self,
            _query: &str,
            _content_type: Option<ContentType>,
        ) -> Result<Vec<ProviderCandidate>, ProviderError> {
            match self.behavior.lock().unwrap().clone() {
                Behavior::Ok(count) => Ok((0..count)
                    .map(|i| ProviderCandidate {
                        provider: self.id.clone(),
                        provider_id: format!("hit-{i}"),
                        title: format!("Title {i}"),
                        content_type: ContentType::Book,
                        release_year: None,
                        cover_url: None,
                        synopsis: None,
                        external_ids: Vec::new(),
                        url: None,
                    })
                    .collect()),
                Behavior::Fail(error) => Err(error),
            }
        }
        async fn get_details(
            &self,
            _id: &str,
        ) -> Result<crate::domain::provider::ProviderMedia, ProviderError> {
            Err(ProviderError::Unsupported {
                provider: self.id.clone(),
                operation: "details".into(),
            })
        }
    }

    /// Fake entry builder: builds fake adapters and records the api_key each
    /// config carries so tests can assert key injection after a rebuild.
    #[derive(Default)]
    struct FakeBuilder {
        behaviors: Mutex<HashMap<String, Behavior>>,
        last_keys: Mutex<Vec<(String, Option<String>)>>,
    }

    impl FakeBuilder {
        fn with(self, id: &str, behavior: Behavior) -> Self {
            self.behaviors.lock().unwrap().insert(id.to_string(), behavior);
            self
        }
        fn recorded_keys(&self) -> Vec<(String, Option<String>)> {
            self.last_keys.lock().unwrap().clone()
        }
    }

    impl EntryBuilder for FakeBuilder {
        fn build(
            &self,
            configs: &[ProviderConfig],
        ) -> Result<Vec<(ProviderConfig, Arc<dyn Provider>)>, String> {
            let mut recorded = self.last_keys.lock().unwrap();
            recorded.clear();
            for config in configs {
                recorded.push((config.id.clone(), config.api_key.clone()));
            }
            drop(recorded);
            configs
                .iter()
                .map(|config| {
                    let behavior = self
                        .behaviors
                        .lock()
                        .unwrap()
                        .get(&config.id)
                        .cloned()
                        .unwrap_or(Behavior::Ok(0));
                    let provider = Arc::new(FakeProvider {
                        id: config.id.clone(),
                        behavior: Mutex::new(behavior),
                    }) as Arc<dyn Provider>;
                    Ok((config.clone(), provider))
                })
                .collect()
        }
    }

    fn service_with(builder: FakeBuilder) -> (ProviderSettingsService, PathBuf) {
        let (dir, file) = temp_settings_file("providers.json");
        let service = ProviderSettingsService::load(
            defaults(),
            file,
            Box::new(InMemoryKeyring::new()),
            Arc::new(builder),
        )
        .expect("service loads");
        (service, dir)
    }

    fn temp_settings_file(name: &str) -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir()
            .join("mylore-test-settings")
            .join(name)
            .parent()
            .unwrap()
            .to_path_buf();
        let file = dir.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        (dir, file)
    }

    #[test]
    fn load_applies_persisted_enabled_and_keyring_keys() {
        let (dir, file) = temp_settings_file("load.db.json");
        std::fs::write(
            &file,
            r#"{ "enabled": { "tmdb": false } }"#,
        )
        .unwrap();
        let store = InMemoryKeyring::new();
        store.set("tmdb", "sekret").unwrap();
        let service = ProviderSettingsService::load(
            defaults(),
            file.clone(),
            Box::new(store),
            Arc::new(FakeBuilder::default()),
        )
        .expect("service loads");

        let tmdb = service.view("tmdb").unwrap();
        assert!(!tmdb.enabled, "persisted disabled flag wins");
        assert!(tmdb.requires_key);
        assert!(tmdb.has_key, "keyring key surfaces as has_key");
        assert!(service.view("openlibrary").unwrap().enabled, "defaults stay on");

        // The coordinator must reflect the persisted disabled flag.
        let providers = service.coordinator().providers();
        assert_eq!(
            providers.iter().find(|p| p.id == "tmdb").unwrap().enabled,
            false
        );
        cleanup_files(&dir);
    }

    #[test]
    fn set_enabled_persists_the_flag_and_rebuilds() {
        let (service, dir) = service_with(FakeBuilder::default());
        service.set_enabled("tmdb", false).expect("toggle");

        let view = service.view("tmdb").unwrap();
        assert!(!view.enabled);
        assert!(
            !service.coordinator().providers().iter().find(|p| p.id == "tmdb").unwrap().enabled
        );

        let persisted =
            std::fs::read_to_string(&service.settings_file).expect("settings file written");
        assert!(persisted.contains("\"tmdb\": false"));
        cleanup_files(&dir);
    }

    #[test]
    fn set_enabled_rejects_unknown_providers() {
        let (service, dir) = service_with(FakeBuilder::default());
        let err = service.set_enabled("nope", true).unwrap_err();
        assert!(matches!(err, AppError::Config(_)));
        cleanup_files(&dir);
    }

    #[test]
    fn set_key_stores_in_keyring_and_injects_into_the_adapter() {
        let store = InMemoryKeyring::new();
        let builder = Arc::new(FakeBuilder::default());
        let (dir, file) = temp_settings_file("set_key.db.json");
        let service = ProviderSettingsService::load(
            defaults(),
            file,
            Box::new(store),
            builder.clone(),
        )
        .expect("service loads");

        let view = service.set_key("tmdb", "abc-123").expect("set key");
        assert!(view.has_key);
        assert_eq!(service.store.get("tmdb").as_deref(), Some("abc-123"));
        assert_eq!(
            builder.recorded_keys(),
            vec![
                ("tmdb".to_string(), Some("abc-123".to_string())),
                ("openlibrary".to_string(), None),
            ],
            "rebuild injects the key into the tmdb adapter only"
        );
        cleanup_files(&dir);
    }

    #[test]
    fn set_key_with_blank_value_clears_the_stored_key() {
        let store = InMemoryKeyring::new();
        let builder = Arc::new(FakeBuilder::default());
        let (dir, file) = temp_settings_file("clear_key.db.json");
        let service = ProviderSettingsService::load(
            defaults(),
            file,
            Box::new(store),
            builder.clone(),
        )
        .expect("service loads");

        service.set_key("tmdb", "abc").expect("set");
        let view = service.set_key("tmdb", "   ").expect("clear");
        assert!(!view.has_key);
        assert_eq!(service.store.get("tmdb"), None);
        assert_eq!(
            builder.recorded_keys(),
            vec![
                ("tmdb".to_string(), None),
                ("openlibrary".to_string(), None),
            ],
            "rebuild drops the cleared key"
        );
        cleanup_files(&dir);
    }

    #[test]
    fn set_key_rejects_providers_that_do_not_use_a_key() {
        let (service, dir) = service_with(FakeBuilder::default());
        let err = service.set_key("openlibrary", "abc").unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
        cleanup_files(&dir);
    }

    #[test]
    fn set_key_rejects_unknown_providers() {
        let (service, dir) = service_with(FakeBuilder::default());
        let err = service.set_key("nope", "abc").unwrap_err();
        assert!(matches!(err, AppError::Config(_)));
        cleanup_files(&dir);
    }

    #[tokio::test]
    async fn test_connection_reports_success_with_result_count() {
        let builder = FakeBuilder::default().with("tmdb", Behavior::Ok(3));
        let (service, dir) = service_with(builder);
        let view = service.test_connection("tmdb").await.expect("ping");
        assert!(view.ok);
        assert_eq!(view.results, 3);
        assert!(view.message.is_empty());
        cleanup_files(&dir);
    }

    #[tokio::test]
    async fn test_connection_reports_provider_errors() {
        let builder = FakeBuilder::default().with(
            "tmdb",
            Behavior::Fail(ProviderError::AuthRequired {
                provider: "tmdb".into(),
            }),
        );
        let (service, dir) = service_with(builder);
        let view = service.test_connection("tmdb").await.expect("ping");
        assert!(!view.ok);
        assert_eq!(view.results, 0);
        assert!(view.message.contains("authentication"), "{}", view.message);
        cleanup_files(&dir);
    }

    #[tokio::test]
    async fn test_connection_runs_even_when_the_provider_is_disabled() {
        let builder = FakeBuilder::default().with("tmdb", Behavior::Ok(1));
        let (service, dir) = service_with(builder);
        service.set_enabled("tmdb", false).unwrap();
        let view = service.test_connection("tmdb").await.expect("ping");
        assert!(view.ok, "disabled providers can still be pinged");
        cleanup_files(&dir);
    }
}
