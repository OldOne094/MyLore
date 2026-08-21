//! Image pipeline commands (MISSION-062). Thin handlers — the download/cache
//! policy lives in `application::image_service`.

use std::sync::Arc;

use tauri::command;
use tauri::State;
use tracing::info;

use crate::application::image_service::{AssetView, ImageService};
use crate::error::AppError;

/// Resolve one asset to a cached view, downloading when the cache policy says
/// so. Statuses: `cached` (local_path usable via `convertFileSrc`), `failed`
/// (transient, retried after a cooldown) and `missing` (permanent broken URL).
#[command]
pub async fn asset_resolve(
    service: State<'_, Arc<ImageService>>,
    asset_id: String,
) -> Result<AssetView, AppError> {
    info!(asset_id, "asset_resolve invoked");
    service.resolve(&asset_id).await
}

/// Resolve many assets in one call (deduped; unknown ids are skipped). The
/// grid/list calls this once per visible page so covers resolve as a batch.
#[command]
pub async fn assets_resolve(
    service: State<'_, Arc<ImageService>>,
    asset_ids: Vec<String>,
) -> Result<Vec<AssetView>, AppError> {
    info!(count = asset_ids.len(), "assets_resolve invoked");
    service.resolve_many(&asset_ids).await
}
