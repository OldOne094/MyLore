//! Import-from-provider commands (MISSION-060). Thin handlers — the import
//! flow (details → identity check → add) lives in `application::import_service`.

use sqlx::SqlitePool;
use tauri::command;
use tauri::State;
use tracing::info;

use std::sync::Arc;

use crate::application::import_service::{ImportService, ProviderImportView};
use crate::application::providers::settings::ProviderSettingsService;
use crate::error::AppError;

/// Import one provider title into the library (search → details → identity
/// check → add). Resolves with the media that owns the title: a new one when
/// nothing matched, or the existing library row when the title was already
/// imported (or strongly duplicates one).
#[command]
pub async fn import_provider(
    state: State<'_, SqlitePool>,
    settings: State<'_, Arc<ProviderSettingsService>>,
    provider: String,
    provider_id: String,
) -> Result<ProviderImportView, AppError> {
    info!(provider, provider_id, "import_provider invoked");
    let service = ImportService::new(state.inner().clone(), settings.coordinator());
    service.import_from_provider(&provider, &provider_id).await
}
