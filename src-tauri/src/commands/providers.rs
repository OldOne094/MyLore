//! Provider settings commands (MISSION-063). Thin handlers — the settings
//! logic (persistence, keyring, coordinator rebuild) lives in
//! `application::providers::settings`.

use std::sync::Arc;

use tauri::State;
use tauri::command;
use tracing::info;

use crate::application::providers::settings::{
    ProviderSettingsService, ProviderSettingsView, ProviderTestView,
};
use crate::error::AppError;

/// Snapshot every registered provider for the settings UI. Resolves with the
/// rows in registration order, or rejects with an AppError string.
#[command]
pub fn providers_list(
    settings: State<'_, Arc<ProviderSettingsService>>,
) -> Result<Vec<ProviderSettingsView>, AppError> {
    info!("providers_list invoked");
    Ok(settings.list())
}

/// Toggle one provider on/off. Persists the flag and takes effect immediately
/// (routing rebuilds the coordinator). Resolves with the updated row or
/// rejects with an AppError string.
#[command]
pub fn provider_set_enabled(
    settings: State<'_, Arc<ProviderSettingsService>>,
    provider: String,
    enabled: bool,
) -> Result<ProviderSettingsView, AppError> {
    info!(provider, enabled, "provider_set_enabled invoked");
    settings.set_enabled(&provider, enabled)
}

/// Store (or clear, when blank) a provider's API key in the OS keyring. The
/// key is never persisted in settings files and never returned to the webview.
/// Resolves with the updated row or rejects with an AppError string.
#[command]
pub fn provider_set_key(
    settings: State<'_, Arc<ProviderSettingsService>>,
    provider: String,
    api_key: String,
) -> Result<ProviderSettingsView, AppError> {
    info!(provider, has_key = !api_key.trim().is_empty(), "provider_set_key invoked");
    settings.set_key(&provider, &api_key)
}

/// Ping one provider with a probe search. Runs even when the provider is
/// disabled so a key can be verified before enabling. Resolves with the test
/// outcome (never rejects for a provider failure) or rejects with an AppError.
#[command]
pub async fn provider_test_connection(
    settings: State<'_, Arc<ProviderSettingsService>>,
    provider: String,
) -> Result<ProviderTestView, AppError> {
    info!(provider, "provider_test_connection invoked");
    settings.test_connection(&provider).await
}
