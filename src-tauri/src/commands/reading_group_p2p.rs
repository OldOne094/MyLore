//! Reading-groups p2p commands (MISSION-115/116). Thin handlers over
//! `application::reading_group_p2p` — the CRDT + E2EE engine — and
//! `application::reading_group_transport` — the relay transport. In a build
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
use crate::application::reading_group_transport::{self, GroupRelayView};
use crate::application::task_service::TaskManager;
use crate::domain::task::TaskSnapshot;
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
/// hand to peers plus the materialized notes. The envelope is queued in the
/// outbox before any relay is contacted (outbox-first), so a later sync
/// publishes it even if the app closes first.
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
    let view = reading_group_p2p::note_edit(
        state.inner(),
        store.inner().as_ref(),
        &group_id,
        &work_key,
        &note_id,
        &body,
    )
    .await?;
    let envelope = reading_group_transport::envelope_bytes(&view.update)?;
    reading_group_transport::enqueue_envelope(state.inner(), &group_id, &work_key, &envelope)
        .await?;
    Ok(view)
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

/// A group's relays and how many envelopes are still waiting to go out.
#[command]
pub async fn reading_group_relays_get(
    state: State<'_, SqlitePool>,
    group_id: String,
) -> Result<GroupRelayView, AppError> {
    info!(group_id, "reading_group_relays_get invoked");
    reading_group_transport::relays_view(state.inner(), &group_id).await
}

/// Replace a group's relay set (owner-only).
#[command]
pub async fn reading_group_relays_set(
    state: State<'_, SqlitePool>,
    group_id: String,
    relays: Vec<String>,
) -> Result<GroupRelayView, AppError> {
    info!(group_id, "reading_group_relays_set invoked");
    reading_group_transport::set_relays(state.inner(), &group_id, relays).await
}

/// Run one relay sync pass (flush the outbox, pull and merge) as a background
/// task. Progress streams over `task_changed`.
#[command]
pub async fn reading_group_sync_now(
    state: State<'_, SqlitePool>,
    store: State<'_, Arc<dyn SecretStore>>,
    tasks: State<'_, Arc<TaskManager>>,
    group_id: String,
) -> Result<TaskSnapshot, AppError> {
    info!(group_id, "reading_group_sync_now invoked");
    reading_group_transport::spawn_sync(state.inner(), store.inner(), tasks.inner(), &group_id)
}
