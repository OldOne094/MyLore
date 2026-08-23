//! Provider settings commands (MISSION-063). Thin handlers — the settings
//! logic (persistence, keyring, coordinator rebuild) lives in
//! `application::providers::settings`.

use std::sync::Arc;

use tauri::command;
use tauri::State;
use tracing::info;

use crate::application::providers::settings::{
    ProviderSettingsService, ProviderSettingsView, ProviderTestView,
};
use crate::error::AppError;

/// Snapshot every registered provider for the settings UI. Resolves with the
/// rows in registration order, or rejects with an AppError string.
#[command]
pub async fn providers_list(
    settings: State<'_, Arc<ProviderSettingsService>>,
) -> Result<Vec<ProviderSettingsView>, AppError> {
    info!("providers_list invoked");
    Ok(settings.list())
}

/// Toggle one provider on/off. Persists the flag and takes effect immediately
/// (routing rebuilds the coordinator). Resolves with the updated row or
/// rejects with an AppError string.
#[command]
pub async fn provider_set_enabled(
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
pub async fn provider_set_key(
    settings: State<'_, Arc<ProviderSettingsService>>,
    provider: String,
    api_key: String,
) -> Result<ProviderSettingsView, AppError> {
    info!(
        provider,
        has_key = !api_key.trim().is_empty(),
        "provider_set_key invoked"
    );
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

/// MISSION-129 — receive the hidden harvester webview's clearance report
/// (UA + cookies). Invoked only from the `nu-fetch` window (capability-
/// scoped); failures are swallowed client-side, so never reject loudly.
#[command]
pub fn nu_clearance_report(payload_json: String) -> Result<(), AppError> {
    #[derive(serde::Deserialize)]
    struct Payload {
        ua: String,
        cookie: String,
        #[serde(default)]
        href: String,
    }
    let payload: Payload = match serde_json::from_str(&payload_json) {
        Ok(p) => p,
        Err(error) => {
            tracing::warn!(%error, first = %payload_json.chars().take(120).collect::<String>(), "malformed NovelUpdates clearance report");
            return Ok(());
        }
    };
    tracing::debug!(
        cookie_len = payload.cookie.len(),
        has_clearance = payload.cookie.contains("cf_clearance"),
        href = %payload.href,
        "NovelUpdates harvester report received"
    );
    if let Some(state) = crate::infrastructure::nu_bridge::global() {
        state.store(payload.ua, payload.cookie);
    }
    Ok(())
}
