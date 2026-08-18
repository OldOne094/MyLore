//! Import commands (MISSION-060 provider import, MISSION-068 file import,
//! MISSION-072 profile exports). Thin handlers — the flows live in
//! `application::import_service` (provider), `application::import_file_service`
//! (JSON/CSV files), and the shared `application::import_pipeline`.

use sqlx::SqlitePool;
use tauri::command;
use tauri::State;
use tracing::info;

use std::sync::Arc;

use crate::application::import_file_service::{ImportFileKind, ImportFileService};
use crate::application::import_pipeline::import_error_to_app;
use crate::application::import_pipeline::ImportPipeline;
use crate::application::import_service::{ImportService, ProviderImportView};
use crate::application::providers::settings::ProviderSettingsService;
use crate::application::task_service::TaskManager;
use crate::domain::import::{ImportPlan, ImportPreview};
use crate::domain::task::{TaskError, TaskKind, TaskSnapshot};
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

/// Sniff a file's import format from its content (MISSION-072): `json` vs
/// `anilist` for JSON files, `csv` vs `goodreads` vs `storygraph` for CSV
/// files. The frontend calls this after reading a file to pick the parser and,
/// for the profile kinds, to skip the column-mapping step.
#[command]
pub fn import_file_detect(source: String) -> Result<String, AppError> {
    info!("import_file_detect invoked");
    ImportFileKind::detect(&source).map(|kind| kind.as_str().to_string())
}

/// Import a file's rows as a background task (MISSION-070): the command spawns
/// the commit on the TaskManager and resolves with the initial snapshot; the
/// UI streams progress via `task_changed` events and can cancel. The commit
/// runs in one transaction, savepoint per row. `plan` selects which source rows
/// to import; null imports every `New` row of the preview. Non-new / invalid /
/// unselected rows are reported as skipped; a row that fails to insert rolls
/// back its own savepoint and is reported as failed. The `ImportReport` is the
/// task's typed result on success.
#[command]
pub async fn import_commit(
    tasks: State<'_, Arc<TaskManager>>,
    state: State<'_, SqlitePool>,
    kind: String,
    source: String,
    mapping: Option<CsvMapping>,
    plan: Option<ImportPlan>,
) -> Result<TaskSnapshot, AppError> {
    info!(kind, "import_commit invoked");
    let kind = kind.parse::<ImportFileKind>()?;
    let pool = state.inner().clone();
    let title = format!("Import {} file", kind.as_str());

    let id = tasks.spawn(TaskKind::ImportFile, title, move |reporter| async move {
        let service = ImportFileService::new(pool.clone());
        let pipeline = ImportPipeline::new(pool);

        reporter.progress(0, Some("Analyzing the file…".to_string()));
        let preview = service
            .preview(kind, &source, mapping.as_ref())
            .await
            .map_err(|error| TaskError::failed(error.to_string()))?;
        let plan = match plan {
            Some(plan) => plan,
            None => ImportPlan::all_new(&preview),
        };

        let commit = pipeline.commit_with_progress(&preview, &plan, |processed, total| {
            let percent = processed
                .checked_mul(100)
                .and_then(|p| p.checked_div(total))
                .unwrap_or(100) as u32;
            reporter.progress(
                percent,
                Some(format!("Importing {processed}/{total} titles")),
            );
        });
        tokio::pin!(commit);

        tokio::select! {
            report = &mut commit => {
                let report = report.map_err(|error| TaskError::failed(error.to_string()))?;
                reporter.progress(100, Some("Import finished".to_string()));
                serde_json::to_value(&report)
                    .map_err(|error| TaskError::failed(error.to_string()))
            }
            _ = reporter.cancelled() => Err(TaskError::Cancelled),
        }
    });

    tasks
        .get(&id)
        .ok_or_else(|| AppError::internal("import task vanished"))
}

/// Read the header row of a CSV for the mapping UI's column pickers
/// (MISSION-068).
#[command]
pub fn import_csv_headers(source: String, delimiter: String) -> Result<Vec<String>, AppError> {
    info!("import_csv_headers invoked");
    csv_headers(&source, &delimiter).map_err(import_error_to_app)
}
