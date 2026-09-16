//! Reading-groups p2p commands (MISSION-115). Thin handlers over
//! `application::reading_group_p2p` — the CRDT + E2EE engine. In a build
//! without the `p2p` feature every command returns a clear "unsupported"
//! validation error; the IPC surface is identical either way.

use std::sync::Arc;

use sqlx::SqlitePool;
use tauri::command;
use tauri::State;
use tracing::info;

use crate::application::reading_group_p2p::{
    self, GroupInviteView, GroupKeyStatus, GroupNoteEntry, GroupNoteSyncView,
};
use crate::error::AppError;
use crate::infrastructure::keyring::SecretStore;

/// Whether this group has a shared key on this device (+ a short fingerprint).
#[command]
pub async fn reading_group_key_status(
    group_id: String,
    store: State<'_, Arc<dyn SecretStore>>,
) -> Result<GroupKeyStatus, AppError> {
    info!(group_id, "reading_group_key_status invoked");
    reading_group_p2p::key_status(store.inner().as_ref(), &group_id).await
}

/// Create an out-of-band invite (generates the group key on first use).
#[command]
pub async fn reading_group_invite_create(
    state: State<'_, SqlitePool>,
    store: State<'_, Arc<dyn SecretStore>>,
    group_id: String,
    relays: Vec<String>,
) -> Result<GroupInviteView, AppError> {
    info!(group_id, "reading_group_invite_create invoked");
    reading_group_p2p::invite_create(state.inner(), store.inner().as_ref(), &group_id, relays).await
}

/// Accept an invite: create the local replica and import the group key.
#[command]
pub async fn reading_group_invite_accept(
    state: State<'_, SqlitePool>,
    store: State<'_, Arc<dyn SecretStore>>,
    link: String,
) -> Result<String, AppError> {
    info!("reading_group_invite_accept invoked");
    reading_group_p2p::invite_accept(state.inner(), store.inner().as_ref(), &link).await
}

/// The group's notes for a work, materialized from the CRDT document.
#[command]
pub async fn reading_group_note_state(
    state: State<'_, SqlitePool>,
    group_id: String,
    work_key: String,
) -> Result<Vec<GroupNoteEntry>, AppError> {
    info!(group_id, work_key, "reading_group_note_state invoked");
    reading_group_p2p::note_state(state.inner(), &group_id, &work_key).await
}

/// Apply a local note edit to the CRDT document; returns the update envelope to
/// hand to peers plus the materialized notes.
#[command]
pub async fn reading_group_note_edit(
    state: State<'_, SqlitePool>,
    store: State<'_, Arc<dyn SecretStore>>,
    group_id: String,
    work_key: String,
    note_id: String,
    body: String,
) -> Result<GroupNoteSyncView, AppError> {
    info!(
        group_id,
        work_key, note_id, "reading_group_note_edit invoked"
    );
    reading_group_p2p::note_edit(
        state.inner(),
        store.inner().as_ref(),
        &group_id,
        &work_key,
        &note_id,
        &body,
    )
    .await
}

/// One sync round-trip: merge an optional remote update and produce the update
/// the peer is missing (diffed against its state vector).
#[command]
pub async fn reading_group_note_sync(
    state: State<'_, SqlitePool>,
    store: State<'_, Arc<dyn SecretStore>>,
    group_id: String,
    work_key: String,
    remote_state_vector: Option<String>,
    remote_update: Option<String>,
) -> Result<GroupNoteSyncView, AppError> {
    info!(group_id, work_key, "reading_group_note_sync invoked");
    reading_group_p2p::note_sync(
        state.inner(),
        store.inner().as_ref(),
        &group_id,
        &work_key,
        remote_state_vector,
        remote_update,
    )
    .await
}
