//! Import commands (MISSION-060 provider import, MISSION-068 file import).
//! Thin handlers — the flows live in `application::import_service` (provider)
//! and `application::import_file_service` (JSON/CSV files).

use sqlx::SqlitePool;
use tauri::command;
use tauri::State;
use tracing::info;

use std::sync::Arc;

use crate::application::import_file_service::{ImportFileKind, ImportFileService};
use crate::application::import_pipeline::import_error_to_app;
use crate::application::import_service::{ImportService, ProviderImportView};
use crate::application::providers::settings::ProviderSettingsService;
use crate::domain::import::{ImportPlan, ImportPreview, ImportReport};
use crate::error::AppError;
use crate::infrastructure::parsers::csv_headers;
use crate::infrastructure::parsers::CsvMapping;

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

/// Parse + dedup a file into a preview (MISSION-068). `mapping` is required
/// for `kind: "csv"` and ignored for `kind: "json"`.
#[command]
pub async fn import_file_preview(
    state: State<'_, SqlitePool>,
    kind: String,
    source: String,
    mapping: Option<CsvMapping>,
) -> Result<ImportPreview, AppError> {
    info!(kind, "import_file_preview invoked");
    let kind = kind.parse::<ImportFileKind>()?;
    let service = ImportFileService::new(state.inner().clone());
    service.preview(kind, &source, mapping.as_ref()).await
}

/// Import a file's rows in one transaction, savepoint per row (MISSION-068).
/// `plan` selects which source rows to import; null imports every `New` row of
/// the preview. Non-new / invalid / unselected rows are reported as skipped; a
/// row that fails to insert rolls back its own savepoint and is reported as
/// failed.
#[command]
pub async fn import_commit(
    state: State<'_, SqlitePool>,
    kind: String,
    source: String,
    mapping: Option<CsvMapping>,
    plan: Option<ImportPlan>,
) -> Result<ImportReport, AppError> {
    info!(kind, "import_commit invoked");
    let kind = kind.parse::<ImportFileKind>()?;
    let service = ImportFileService::new(state.inner().clone());
    service
        .commit(kind, &source, mapping.as_ref(), plan.as_ref())
        .await
}

/// Read the header row of a CSV for the mapping UI's column pickers
/// (MISSION-068).
#[command]
pub fn import_csv_headers(source: String, delimiter: String) -> Result<Vec<String>, AppError> {
    info!("import_csv_headers invoked");
    csv_headers(&source, &delimiter).map_err(import_error_to_app)
}
