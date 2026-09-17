//! Reading-group commands (MISSION-114). Thin handlers over
//! `application::reading_group_service` — local CRUD for groups/members/shelf/
//! notes plus the manual `group_state.json` export/import seam.

use sqlx::SqlitePool;
use tauri::command;
use tauri::State;
use tracing::info;

use crate::application::reading_group_service::{
    GroupExportReport, GroupImportReport, GroupNoteView, GroupPrefs, GroupShelfEntryView,
    GroupView, ReadingGroupService,
};
use crate::domain::reading_group::work_key;
use crate::domain::value_objects::{ExternalId, ProviderId};
use crate::error::AppError;

/// The stable cross-device key for a work (MISSION-117).
///
/// The UI must not re-implement the fold + hash: a second implementation would
/// drift from the domain's and silently split a work in two across devices.
/// Exposed so the frontend always asks the same code path.
#[command]
pub async fn reading_group_work_key(
    title: String,
    author: Option<String>,
    year: Option<i64>,
    provider: Option<String>,
    external_id: Option<String>,
) -> Result<String, AppError> {
    info!(title, "reading_group_work_key invoked");
    let external_ids = match (provider, external_id) {
        (Some(provider), Some(value)) if !value.trim().is_empty() => {
            let provider =
                ProviderId::new(provider.trim().to_lowercase()).map_err(AppError::from)?;
            vec![ExternalId::new(provider, value.trim(), None).map_err(AppError::from)?]
        }
        _ => Vec::new(),
    };
    Ok(work_key(&external_ids, &title, author.as_deref(), year))
}

/// The local opt-in flag + identity (mints a member id when absent).
#[command]
pub async fn reading_group_prefs_get(state: State<'_, SqlitePool>) -> Result<GroupPrefs, AppError> {
    info!("reading_group_prefs_get invoked");
    ReadingGroupService::new(state.inner().clone())
        .prefs()
        .await
}

/// Persist the opt-in flag and display name.
#[command]
pub async fn reading_group_prefs_set(
    state: State<'_, SqlitePool>,
    enabled: bool,
    display_name: String,
) -> Result<GroupPrefs, AppError> {
    info!(enabled, "reading_group_prefs_set invoked");
    ReadingGroupService::new(state.inner().clone())
        .set_prefs(enabled, &display_name)
        .await
}

/// Every group with its members and row counts.
#[command]
pub async fn reading_group_list(state: State<'_, SqlitePool>) -> Result<Vec<GroupView>, AppError> {
    info!("reading_group_list invoked");
    ReadingGroupService::new(state.inner().clone())
        .list_groups()
        .await
}

/// Create a group (seeds the local owner membership).
#[command]
pub async fn reading_group_create(
    state: State<'_, SqlitePool>,
    name: String,
) -> Result<GroupView, AppError> {
    info!(name, "reading_group_create invoked");
    ReadingGroupService::new(state.inner().clone())
        .create_group(&name)
        .await
}

#[command]
pub async fn reading_group_rename(
    state: State<'_, SqlitePool>,
    group_id: String,
    name: String,
) -> Result<GroupView, AppError> {
    info!(group_id, name, "reading_group_rename invoked");
    ReadingGroupService::new(state.inner().clone())
        .rename_group(&group_id, &name)
        .await
}

#[command]
pub async fn reading_group_delete(
    state: State<'_, SqlitePool>,
    group_id: String,
) -> Result<(), AppError> {
    info!(group_id, "reading_group_delete invoked");
    ReadingGroupService::new(state.inner().clone())
        .delete_group(&group_id)
        .await
}

#[command]
pub async fn reading_group_view(
    state: State<'_, SqlitePool>,
    group_id: String,
) -> Result<GroupView, AppError> {
    info!(group_id, "reading_group_view invoked");
    ReadingGroupService::new(state.inner().clone())
        .view_group(&group_id)
        .await
}

#[command]
pub async fn reading_group_add_member(
    state: State<'_, SqlitePool>,
    group_id: String,
    member_id: String,
    display_name: String,
    role: String,
) -> Result<GroupView, AppError> {
    info!(group_id, member_id, "reading_group_add_member invoked");
    ReadingGroupService::new(state.inner().clone())
        .add_member(&group_id, &member_id, &display_name, &role)
        .await
}

#[command]
pub async fn reading_group_remove_member(
    state: State<'_, SqlitePool>,
    store: State<'_, std::sync::Arc<dyn crate::infrastructure::keyring::SecretStore>>,
    group_id: String,
    member_id: String,
) -> Result<GroupView, AppError> {
    info!(group_id, member_id, "reading_group_remove_member invoked");
    let view = ReadingGroupService::new(state.inner().clone())
        .remove_member(&group_id, &member_id)
        .await?;
    // Removing a member is only meaningful with forward secrecy: rotate the group
    // key so they cannot read anything sealed from now on (MISSION-118). The
    // members who stay need the new invite; the UI says so. A build without the
    // `p2p` feature (or a group that never had a key) has nothing to rotate.
    crate::application::reading_group_p2p::rotate_after_removal(
        state.inner(),
        store.inner().as_ref(),
        &group_id,
    )
    .await?;
    Ok(view)
}

#[command]
pub async fn reading_group_shelf(
    state: State<'_, SqlitePool>,
    group_id: String,
    member_id: Option<String>,
) -> Result<Vec<GroupShelfEntryView>, AppError> {
    info!(group_id, "reading_group_shelf invoked");
    ReadingGroupService::new(state.inner().clone())
        .shelf(&group_id, member_id.as_deref())
        .await
}

#[allow(clippy::too_many_arguments)]
#[command]
pub async fn reading_group_set_shelf(
    state: State<'_, SqlitePool>,
    group_id: String,
    member_id: String,
    work_key: String,
    title: String,
    content_type: String,
    status: String,
    progress: i64,
) -> Result<GroupShelfEntryView, AppError> {
    info!(
        group_id,
        member_id, work_key, "reading_group_set_shelf invoked"
    );
    ReadingGroupService::new(state.inner().clone())
        .set_shelf_entry(
            &group_id,
            &member_id,
            &work_key,
            &title,
            &content_type,
            &status,
            progress,
        )
        .await
}

#[command]
pub async fn reading_group_notes(
    state: State<'_, SqlitePool>,
    group_id: String,
    work_key: Option<String>,
) -> Result<Vec<GroupNoteView>, AppError> {
    info!(group_id, "reading_group_notes invoked");
    ReadingGroupService::new(state.inner().clone())
        .notes(&group_id, work_key.as_deref())
        .await
}

#[command]
pub async fn reading_group_add_note(
    state: State<'_, SqlitePool>,
    group_id: String,
    work_key: String,
    author_id: String,
    body: String,
) -> Result<GroupNoteView, AppError> {
    info!(group_id, work_key, "reading_group_add_note invoked");
    ReadingGroupService::new(state.inner().clone())
        .add_note(&group_id, &work_key, &author_id, &body)
        .await
}

#[command]
pub async fn reading_group_update_note(
    state: State<'_, SqlitePool>,
    note_id: String,
    body: String,
) -> Result<GroupNoteView, AppError> {
    info!(note_id, "reading_group_update_note invoked");
    ReadingGroupService::new(state.inner().clone())
        .update_note(&note_id, &body)
        .await
}

#[command]
pub async fn reading_group_delete_note(
    state: State<'_, SqlitePool>,
    note_id: String,
) -> Result<(), AppError> {
    info!(note_id, "reading_group_delete_note invoked");
    ReadingGroupService::new(state.inner().clone())
        .delete_note(&note_id)
        .await
}

/// Write `group_state.json` (one group when `group_id` is given, else all) to
/// a caller-chosen path.
#[command]
pub async fn reading_group_export(
    state: State<'_, SqlitePool>,
    path: String,
    group_id: Option<String>,
) -> Result<GroupExportReport, AppError> {
    info!(path, "reading_group_export invoked");
    ReadingGroupService::new(state.inner().clone())
        .write_state(std::path::Path::new(&path), group_id.as_deref())
        .await
}

/// Merge a `group_state.json` payload (read in the webview) into the library.
#[command]
pub async fn reading_group_import(
    state: State<'_, SqlitePool>,
    source: String,
) -> Result<GroupImportReport, AppError> {
    info!("reading_group_import invoked");
    ReadingGroupService::new(state.inner().clone())
        .import_state(&source)
        .await
}
