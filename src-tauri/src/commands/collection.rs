//! Collection commands (MISSION-076, MISSION-077). Thin handlers over
//! `application::collection_service` — CRUD over the user's collections plus
//! ordered membership (bulk add, single remove, drag/drop reorder) and, since
//! MISSION-077, smart collections built from a saved library filter.

use sqlx::SqlitePool;
use tauri::command;
use tauri::State;
use tracing::info;

use crate::application::bulk_service::{resolve_targets, BulkFilter, BulkResult};
use crate::application::collection_service::{
    CollectionMemberView, CollectionService, CollectionView, SmartFilter,
};
use crate::error::AppError;

/// List collections with member counts, for the Collections page and the
/// add-to-list picker.
#[command]
pub async fn collection_list(
    state: State<'_, SqlitePool>,
) -> Result<Vec<CollectionView>, AppError> {
    info!("collection_list invoked");
    let service = CollectionService::new(state.inner().clone());
    service.list().await
}

/// Create a manual collection; resolves with its view.
#[command]
pub async fn collection_create(
    state: State<'_, SqlitePool>,
    name: String,
) -> Result<CollectionView, AppError> {
    info!(name, "collection_create invoked");
    let service = CollectionService::new(state.inner().clone());
    service.create(&name).await
}

/// Rename a collection; resolves with the updated view.
#[command]
pub async fn collection_rename(
    state: State<'_, SqlitePool>,
    collection_id: String,
    name: String,
) -> Result<CollectionView, AppError> {
    info!(collection_id, name, "collection_rename invoked");
    let service = CollectionService::new(state.inner().clone());
    service.rename(&collection_id, &name).await
}

/// Create a smart collection from a saved filter; membership is computed live.
#[command]
pub async fn collection_create_smart(
    state: State<'_, SqlitePool>,
    name: String,
    filter: SmartFilter,
) -> Result<CollectionView, AppError> {
    info!(name, "collection_create_smart invoked");
    let service = CollectionService::new(state.inner().clone());
    service.create_smart(&name, &filter).await
}

/// Replace a smart collection's filter; resolves with the updated view.
#[command]
pub async fn collection_update_smart(
    state: State<'_, SqlitePool>,
    collection_id: String,
    filter: SmartFilter,
) -> Result<CollectionView, AppError> {
    info!(collection_id, "collection_update_smart invoked");
    let service = CollectionService::new(state.inner().clone());
    service.update_smart_filter(&collection_id, &filter).await
}

/// Delete a collection; members cascade. Resolves with the removed name.
#[command]
pub async fn collection_delete(
    state: State<'_, SqlitePool>,
    collection_id: String,
) -> Result<String, AppError> {
    info!(collection_id, "collection_delete invoked");
    let service = CollectionService::new(state.inner().clone());
    service.delete(&collection_id).await
}

/// A collection's members in display order.
#[command]
pub async fn collection_members(
    state: State<'_, SqlitePool>,
    collection_id: String,
) -> Result<Vec<CollectionMemberView>, AppError> {
    info!(collection_id, "collection_members invoked");
    let service = CollectionService::new(state.inner().clone());
    service.members(&collection_id).await
}

/// Add many media to one collection (idempotent append). An optional facet
/// `filter` resolves the media set server-side (MISSION-078); the result is a
/// per-item summary.
#[command]
pub async fn collection_bulk_add(
    state: State<'_, SqlitePool>,
    collection_id: String,
    media_ids: Vec<String>,
    filter: Option<BulkFilter>,
) -> Result<BulkResult, AppError> {
    info!(
        collection_id,
        count = media_ids.len(),
        filtered = filter.is_some(),
        "collection_bulk_add invoked"
    );
    let pool = state.inner().clone();
    let targets = resolve_targets(&pool, filter.as_ref(), &media_ids).await?;
    let service = CollectionService::new(pool);
    service.add_members(&collection_id, &targets).await
}

/// Remove one media from a collection; resolves with the removed media id.
#[command]
pub async fn collection_remove_member(
    state: State<'_, SqlitePool>,
    collection_id: String,
    media_id: String,
) -> Result<String, AppError> {
    info!(collection_id, media_id, "collection_remove_member invoked");
    let service = CollectionService::new(state.inner().clone());
    service.remove_member(&collection_id, &media_id).await?;
    Ok(media_id)
}

/// Persist a drag/drop reorder of a collection's members. The provided media
/// ids must be exactly the current members; positions are rewritten 0..n.
#[command]
pub async fn collection_reorder(
    state: State<'_, SqlitePool>,
    collection_id: String,
    media_ids: Vec<String>,
) -> Result<(), AppError> {
    info!(
        collection_id,
        count = media_ids.len(),
        "collection_reorder invoked"
    );
    let service = CollectionService::new(state.inner().clone());
    service.reorder(&collection_id, &media_ids).await
}
