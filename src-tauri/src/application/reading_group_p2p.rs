//! Reading-groups CRDT + E2EE engine (MISSION-115).
//!
//! Everything here is behind the `p2p` cargo feature. A default build ships a
//! stub whose functions return a clear "built without p2p" error, so the IPC
//! surface is identical but nothing heavy is compiled in.
//!
//! **Scope.** This is the *engine*: group-key lifecycle, XChaCha20-Poly1305
//! sealing of sync envelopes, the out-of-band invite (key + relays), and the
//! conflict-free notes document (a `yrs` map of note id → body) with a
//! state-vector handshake so only missing updates travel. The Nostr transport
//! (MISSION-116) and the UI (MISSION-117) build on these primitives.
//!
//! **What E2EE protects.** Envelopes that leave the device (invite payload and
//! sync updates) are encrypted with the group key. The locally stored document
//! snapshot is plaintext like the rest of the database — at-rest protection is
//! SQLCipher's job (MISSION-112), not this layer's.

/// Whether this group has a shared key on this device, plus a short fingerprint.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GroupKeyStatus {
    pub has_key: bool,
    pub key_id: Option<String>,
}

/// An out-of-band invite (QR payload + shareable link).
#[derive(Debug, Clone, serde::Serialize)]
pub struct GroupInviteView {
    pub group_id: String,
    pub group_name: String,
    pub epoch: i64,
    pub relays: Vec<String>,
    /// `mylore://group-invite#<base64url(payload)>` — one string to share.
    pub link: String,
    /// The raw JSON payload (for rendering a QR code locally).
    pub qr_payload: String,
    /// Short fingerprint of the shared key (safe to show).
    pub key_id: String,
}

/// One materialized note from the conflict-free document.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct GroupNoteEntry {
    pub note_id: String,
    pub body: String,
}

/// The result of one sync round-trip.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GroupNoteSyncView {
    pub group_id: String,
    pub work_key: String,
    /// This peer's state vector (base64) — send it so the peer can diff.
    pub state_vector: String,
    /// Encrypted update carrying everything the peer is missing (base64).
    pub update: String,
    /// The document's notes after merging.
    pub notes: Vec<GroupNoteEntry>,
    /// Whether the document was compacted during this call.
    pub compacted: bool,
}

#[cfg(feature = "p2p")]
mod imp {
    use super::*;

    use base64::Engine as _;
    use chacha20poly1305::aead::{Aead, KeyInit, Payload};
    use chacha20poly1305::{XChaCha20Poly1305, XNonce};
    use sqlx::SqlitePool;
    use yrs::updates::decoder::Decode;
    use yrs::updates::encoder::Encode;
    use yrs::{Doc, Map, ReadTxn, StateVector, Transact, Update};

    use crate::error::AppError;
    use crate::infrastructure::keyring::SecretStore;
    use crate::infrastructure::repositories::reading_group as repo;
    use crate::infrastructure::repositories::reading_group::DocRecord;

    const KEY_PREFIX: &str = "readingGroup.key.";
    const DOC_MAP: &str = "notes";
    /// Compact after this many applied updates…
    const MAX_PENDING_OPS: i64 = 100;
    /// …or after this many days without compaction.
    const COMPACT_AFTER_DAYS: i64 = 7;
    const INVITE_FORMAT: &str = "mylore.group-invite";
    const INVITE_VERSION: u32 = 1;
    const LINK_PREFIX: &str = "mylore://group-invite#";

    #[derive(serde::Serialize, serde::Deserialize)]
    struct InvitePayload {
        format: String,
        version: u32,
        group_id: String,
        group_name: String,
        epoch: i64,
        key: String,
        relays: Vec<String>,
    }

    fn b64(bytes: &[u8]) -> String {
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    }

    /// Fill `dest` with OS randomness (`getrandom`), which backs both the group
    /// key and every nonce.
    fn fill_random(dest: &mut [u8]) -> Result<(), AppError> {
        getrandom::fill(dest).map_err(|e| AppError::internal(format!("no OS randomness: {e}")))
    }

    fn unb64(text: &str) -> Result<Vec<u8>, AppError> {
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(text.as_bytes())
            .map_err(|e| AppError::validation(format!("invalid base64 payload: {e}")))
    }

    // ------------------------------------------------------------- group key

    fn key_entry(group_id: &str) -> String {
        format!("{KEY_PREFIX}{group_id}")
    }

    /// A short, non-secret fingerprint of the key (first 6 bytes, hex).
    fn fingerprint(key: &[u8; 32]) -> String {
        key.iter().take(6).map(|b| format!("{b:02x}")).collect()
    }

    fn load_key(store: &dyn SecretStore, group_id: &str) -> Result<Option<[u8; 32]>, AppError> {
        let Some(raw) = store
            .get(&key_entry(group_id))
            .map_err(AppError::internal)?
        else {
            return Ok(None);
        };
        let bytes = unb64(&raw)?;
        let key: [u8; 32] = bytes
            .as_slice()
            .try_into()
            .map_err(|_| AppError::internal("stored group key has the wrong length"))?;
        Ok(Some(key))
    }

    /// Load the group key, generating + persisting one on first use.
    fn ensure_key(store: &dyn SecretStore, group_id: &str) -> Result<[u8; 32], AppError> {
        if let Some(key) = load_key(store, group_id)? {
            return Ok(key);
        }
        let mut key = [0u8; 32];
        fill_random(&mut key)?;
        store
            .set(&key_entry(group_id), &b64(&key))
            .map_err(AppError::internal)?;
        Ok(key)
    }

    /// Seed a key from an accepted invite (no-op when a key already exists).
    fn store_key(store: &dyn SecretStore, group_id: &str, key: &[u8; 32]) -> Result<(), AppError> {
        store
            .set(&key_entry(group_id), &b64(key))
            .map_err(AppError::internal)
    }

    // ---------------------------------------------------------------- sealing

    fn aad(group_id: &str, epoch: i64) -> Vec<u8> {
        format!("{group_id}|{epoch}").into_bytes()
    }

    fn seal(key: &[u8; 32], aad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, AppError> {
        let cipher = XChaCha20Poly1305::new_from_slice(key)
            .map_err(|_| AppError::internal("invalid group key"))?;
        let mut nonce = [0u8; 24];
        fill_random(&mut nonce)?;
        let nonce =
            XNonce::try_from(&nonce[..]).map_err(|_| AppError::internal("invalid nonce length"))?;
        let ciphertext = cipher
            .encrypt(
                &nonce,
                Payload {
                    msg: plaintext,
                    aad,
                },
            )
            .map_err(|_| AppError::internal("encryption failed"))?;
        let mut envelope = Vec::with_capacity(24 + ciphertext.len());
        envelope.extend_from_slice(&nonce);
        envelope.extend_from_slice(&ciphertext);
        Ok(envelope)
    }

    fn open(key: &[u8; 32], aad: &[u8], envelope: &[u8]) -> Result<Vec<u8>, AppError> {
        if envelope.len() <= 24 {
            return Err(AppError::validation("group envelope is truncated"));
        }
        let cipher = XChaCha20Poly1305::new_from_slice(key)
            .map_err(|_| AppError::internal("invalid group key"))?;
        let (nonce, ciphertext) = envelope.split_at(24);
        let nonce =
            XNonce::try_from(nonce).map_err(|_| AppError::internal("invalid nonce length"))?;
        cipher
            .decrypt(
                &nonce,
                Payload {
                    msg: ciphertext,
                    aad,
                },
            )
            .map_err(|_| AppError::validation("group envelope failed to decrypt"))
    }

    // ------------------------------------------------------------------ notes

    fn open_doc(state: &[u8]) -> Result<Doc, AppError> {
        let doc = Doc::new();
        if !state.is_empty() {
            let update = Update::decode_v1(state)
                .map_err(|e| AppError::internal(format!("crdt decode failed: {e}")))?;
            doc.transact_mut()
                .apply_update(update)
                .map_err(|e| AppError::internal(format!("crdt apply failed: {e}")))?;
        }
        Ok(doc)
    }

    fn encode_full_state(doc: &Doc) -> Vec<u8> {
        doc.transact()
            .encode_state_as_update_v1(&StateVector::default())
    }

    fn set_note(doc: &Doc, note_id: &str, body: &str) {
        let map = doc.get_or_insert_map(DOC_MAP);
        let mut txn = doc.transact_mut();
        map.insert(&mut txn, note_id, body);
    }

    fn entries(doc: &Doc) -> Vec<GroupNoteEntry> {
        let map = doc.get_or_insert_map(DOC_MAP);
        let txn = doc.transact();
        let mut notes: Vec<GroupNoteEntry> = map
            .iter(&txn)
            .map(|(note_id, body)| GroupNoteEntry {
                note_id: note_id.to_string(),
                body: body.to_string(&txn),
            })
            .collect();
        notes.sort_by(|a, b| a.note_id.cmp(&b.note_id));
        notes
    }

    fn should_compact(pending_ops: i64, compacted_at: Option<&str>) -> bool {
        if pending_ops >= MAX_PENDING_OPS {
            return true;
        }
        match compacted_at {
            Some(stamp) => chrono::DateTime::parse_from_rfc3339(stamp)
                .map(|t| {
                    (chrono::Utc::now() - t.with_timezone(&chrono::Utc)).num_days()
                        >= COMPACT_AFTER_DAYS
                })
                .unwrap_or(false),
            None => false,
        }
    }

    // ---------------------------------------------------------------- use-cases

    pub async fn key_status(
        store: &dyn SecretStore,
        group_id: &str,
    ) -> Result<GroupKeyStatus, AppError> {
        let key = load_key(store, group_id)?;
        Ok(GroupKeyStatus {
            has_key: key.is_some(),
            key_id: key.as_ref().map(fingerprint),
        })
    }

    /// Create an out-of-band invite for a group (generates the key on first use).
    pub async fn invite_create(
        pool: &SqlitePool,
        store: &dyn SecretStore,
        group_id: &str,
        relays: Vec<String>,
    ) -> Result<GroupInviteView, AppError> {
        let group = repo::get_group(pool, group_id)
            .await?
            .ok_or_else(|| AppError::validation(format!("unknown group: {group_id}")))?;
        let key = ensure_key(store, group_id)?;
        let payload = InvitePayload {
            format: INVITE_FORMAT.to_string(),
            version: INVITE_VERSION,
            group_id: group.id.clone(),
            group_name: group.name.clone(),
            epoch: group.epoch,
            key: b64(&key),
            relays: normalize_relays(relays),
        };
        let json = serde_json::to_string(&payload)?;
        Ok(GroupInviteView {
            group_id: group.id,
            group_name: group.name,
            epoch: group.epoch,
            relays: payload.relays,
            link: format!("{LINK_PREFIX}{}", b64(json.as_bytes())),
            qr_payload: json,
            key_id: fingerprint(&key),
        })
    }

    /// Accept an invite: create a local replica of the group and import its key.
    /// Returns the group id.
    pub async fn invite_accept(
        pool: &SqlitePool,
        store: &dyn SecretStore,
        link: &str,
    ) -> Result<String, AppError> {
        let payload = parse_invite(link)?;
        let key: [u8; 32] = unb64(&payload.key)?
            .as_slice()
            .try_into()
            .map_err(|_| AppError::validation("invite key has the wrong length"))?;

        // Create the local replica when it isn't already present.
        if repo::get_group(pool, &payload.group_id).await?.is_none() {
            let now = chrono::Utc::now().to_rfc3339();
            let member_id = format!("m-{}", uuid::Uuid::new_v4());
            let mut tx = pool.begin().await?;
            repo::create_group(
                &mut *tx,
                &repo::GroupRecord {
                    id: payload.group_id.clone(),
                    name: payload.group_name.clone(),
                    owner_id: member_id.clone(),
                    epoch: payload.epoch,
                    created_at: now.clone(),
                    updated_at: now.clone(),
                },
            )
            .await?;
            repo::add_member(
                &mut *tx,
                &repo::MemberRecord {
                    group_id: payload.group_id.clone(),
                    member_id,
                    display_name: "Me".to_string(),
                    role: "owner".to_string(),
                    joined_at: now,
                },
            )
            .await?;
            tx.commit().await?;
        }
        store_key(store, &payload.group_id, &key)?;
        Ok(payload.group_id)
    }

    /// One sync round-trip: merge an optional remote update, then produce the
    /// update the peer is missing (diffed against its state vector).
    #[allow(clippy::too_many_arguments)]
    pub async fn note_sync(
        pool: &SqlitePool,
        store: &dyn SecretStore,
        group_id: &str,
        work_key: &str,
        remote_state_vector: Option<String>,
        remote_update: Option<String>,
    ) -> Result<GroupNoteSyncView, AppError> {
        let group = repo::get_group(pool, group_id)
            .await?
            .ok_or_else(|| AppError::validation(format!("unknown group: {group_id}")))?;
        let key = load_key(store, group_id)?.ok_or_else(|| {
            AppError::validation("no group key — create or accept an invite first")
        })?;
        let aad = aad(group_id, group.epoch);

        let existing = repo::get_doc(pool, group_id, work_key).await?;
        let doc = open_doc(existing.as_ref().map(|d| d.state.as_slice()).unwrap_or(&[]))?;

        // 1. Merge the peer's update when provided.
        let mut applied = false;
        if let Some(update_b64) = remote_update {
            let envelope = unb64(&update_b64)?;
            let plaintext = open(&key, &aad, &envelope)?;
            let update = Update::decode_v1(&plaintext)
                .map_err(|e| AppError::validation(format!("invalid group update: {e}")))?;
            doc.transact_mut()
                .apply_update(update)
                .map_err(|e| AppError::validation(format!("group update rejected: {e}")))?;
            applied = true;
        }

        // 2. Produce the peer's missing update (or the full state when it sent
        //    no state vector — a first sync).
        let peer_sv = match remote_state_vector {
            Some(sv) => StateVector::decode_v1(&unb64(&sv)?)
                .map_err(|e| AppError::validation(format!("invalid state vector: {e}")))?,
            None => StateVector::default(),
        };
        let own_state_vector = doc.transact().state_vector().encode_v1();
        let update_plain = doc.transact().encode_state_as_update_v1(&peer_sv);
        let envelope = seal(&key, &aad, &update_plain)?;

        // 3. Persist, compacting on the size/time policy.
        let now = chrono::Utc::now().to_rfc3339();
        let pending_ops =
            existing.as_ref().map(|d| d.pending_ops).unwrap_or(0) + i64::from(applied);
        let compacted_at = existing.as_ref().and_then(|d| d.compacted_at.clone());
        let compacted = should_compact(pending_ops, compacted_at.as_deref());
        let (pending_ops, compacted_at, state) = if compacted {
            (0, Some(now.clone()), encode_full_state(&doc))
        } else {
            (pending_ops, compacted_at, encode_full_state(&doc))
        };
        repo::upsert_doc(
            pool,
            &DocRecord {
                group_id: group_id.to_string(),
                work_key: work_key.to_string(),
                state,
                pending_ops,
                compacted_at,
                updated_at: now,
            },
        )
        .await?;

        Ok(GroupNoteSyncView {
            group_id: group_id.to_string(),
            work_key: work_key.to_string(),
            state_vector: b64(&own_state_vector),
            update: b64(&envelope),
            notes: entries(&doc),
            compacted,
        })
    }

    /// The document's notes, materialized for the UI.
    pub async fn note_state(
        pool: &SqlitePool,
        group_id: &str,
        work_key: &str,
    ) -> Result<Vec<GroupNoteEntry>, AppError> {
        let existing = repo::get_doc(pool, group_id, work_key).await?;
        let doc = open_doc(existing.as_ref().map(|d| d.state.as_slice()).unwrap_or(&[]))?;
        Ok(entries(&doc))
    }

    /// Apply a local note edit to the document and return its update envelope to
    /// hand to peers (plus the materialized notes).
    pub async fn note_edit(
        pool: &SqlitePool,
        store: &dyn SecretStore,
        group_id: &str,
        work_key: &str,
        note_id: &str,
        body: &str,
    ) -> Result<GroupNoteSyncView, AppError> {
        if note_id.trim().is_empty() {
            return Err(AppError::validation("note id must not be empty"));
        }
        let group = repo::get_group(pool, group_id)
            .await?
            .ok_or_else(|| AppError::validation(format!("unknown group: {group_id}")))?;
        // A local edit mints the group key on first use (an invite shares it).
        let key = ensure_key(store, group_id)?;
        let aad = aad(group_id, group.epoch);

        let existing = repo::get_doc(pool, group_id, work_key).await?;
        let doc = open_doc(existing.as_ref().map(|d| d.state.as_slice()).unwrap_or(&[]))?;
        set_note(&doc, note_id.trim(), body);

        let now = chrono::Utc::now().to_rfc3339();
        let update_plain = encode_full_state(&doc);
        let envelope = seal(&key, &aad, &update_plain)?;
        let own_state_vector = doc.transact().state_vector().encode_v1();

        let pending_ops = existing.as_ref().map(|d| d.pending_ops).unwrap_or(0) + 1;
        let compacted_at = existing.as_ref().and_then(|d| d.compacted_at.clone());
        let compacted = should_compact(pending_ops, compacted_at.as_deref());
        let (pending_ops, compacted_at) = if compacted {
            (0, Some(now.clone()))
        } else {
            (pending_ops, compacted_at)
        };
        repo::upsert_doc(
            pool,
            &DocRecord {
                group_id: group_id.to_string(),
                work_key: work_key.to_string(),
                state: encode_full_state(&doc),
                pending_ops,
                compacted_at,
                updated_at: now,
            },
        )
        .await?;

        Ok(GroupNoteSyncView {
            group_id: group_id.to_string(),
            work_key: work_key.to_string(),
            state_vector: b64(&own_state_vector),
            update: b64(&envelope),
            notes: entries(&doc),
            compacted,
        })
    }

    fn parse_invite(link: &str) -> Result<InvitePayload, AppError> {
        let body = link
            .trim()
            .strip_prefix(LINK_PREFIX)
            .ok_or_else(|| AppError::validation("not a MyLore group invite link"))?;
        let json = String::from_utf8(unb64(body)?)
            .map_err(|_| AppError::validation("invite payload is not UTF-8"))?;
        let payload: InvitePayload = serde_json::from_str(&json)
            .map_err(|e| AppError::validation(format!("invalid invite payload: {e}")))?;
        if payload.format != INVITE_FORMAT {
            return Err(AppError::validation("not a MyLore group invite"));
        }
        if payload.version != INVITE_VERSION {
            return Err(AppError::validation(format!(
                "unsupported invite version: {}",
                payload.version
            )));
        }
        Ok(payload)
    }

    fn normalize_relays(relays: Vec<String>) -> Vec<String> {
        let mut relays: Vec<String> = relays
            .into_iter()
            .map(|r| r.trim().to_string())
            .filter(|r| !r.is_empty())
            .collect();
        relays.sort();
        relays.dedup();
        relays.truncate(8);
        relays
    }

    /// Exposed for tests: the invite link/parse round-trip without a DB.
    #[cfg(test)]
    pub fn invite_link_round_trip_for_test(link: &str) -> Result<(String, [u8; 32]), AppError> {
        let payload = parse_invite(link)?;
        let key: [u8; 32] = unb64(&payload.key)?
            .as_slice()
            .try_into()
            .map_err(|_| AppError::validation("invite key has the wrong length"))?;
        Ok((payload.group_id, key))
    }

    /// Exposed for tests: seal with one AAD, open with another.
    #[cfg(test)]
    pub fn seal_for_test(key: &[u8; 32], aad: &[u8], msg: &[u8]) -> Result<Vec<u8>, AppError> {
        seal(key, aad, msg)
    }

    #[cfg(test)]
    pub fn open_for_test(key: &[u8; 32], aad: &[u8], envelope: &[u8]) -> Result<Vec<u8>, AppError> {
        open(key, aad, envelope)
    }
}

#[cfg(not(feature = "p2p"))]
mod imp {
    use super::*;
    use sqlx::SqlitePool;

    use crate::error::AppError;
    use crate::infrastructure::keyring::SecretStore;

    fn unsupported<T>() -> Result<T, AppError> {
        Err(AppError::validation(
            "this build was compiled without reading-groups p2p support (feature `p2p`)",
        ))
    }

    pub async fn key_status(
        _store: &dyn SecretStore,
        _group_id: &str,
    ) -> Result<GroupKeyStatus, AppError> {
        unsupported()
    }

    pub async fn invite_create(
        _pool: &SqlitePool,
        _store: &dyn SecretStore,
        _group_id: &str,
        _relays: Vec<String>,
    ) -> Result<GroupInviteView, AppError> {
        unsupported()
    }

    pub async fn invite_accept(
        _pool: &SqlitePool,
        _store: &dyn SecretStore,
        _link: &str,
    ) -> Result<String, AppError> {
        unsupported()
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn note_sync(
        _pool: &SqlitePool,
        _store: &dyn SecretStore,
        _group_id: &str,
        _work_key: &str,
        _remote_state_vector: Option<String>,
        _remote_update: Option<String>,
    ) -> Result<GroupNoteSyncView, AppError> {
        unsupported()
    }

    pub async fn note_state(
        _pool: &SqlitePool,
        _group_id: &str,
        _work_key: &str,
    ) -> Result<Vec<GroupNoteEntry>, AppError> {
        unsupported()
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn note_edit(
        _pool: &SqlitePool,
        _store: &dyn SecretStore,
        _group_id: &str,
        _work_key: &str,
        _note_id: &str,
        _body: &str,
    ) -> Result<GroupNoteSyncView, AppError> {
        unsupported()
    }
}

pub use imp::{invite_accept, invite_create, key_status, note_edit, note_state, note_sync};

#[cfg(all(test, feature = "p2p"))]
mod tests {
    use super::*;
    use crate::application::reading_group_service::ReadingGroupService;
    use crate::infrastructure::keyring::InMemoryKeyring;
    use crate::infrastructure::test_support::{cleanup_files, migrated_pool};

    /// One "device": its own DB + secret store.
    struct Device {
        pool: sqlx::SqlitePool,
        store: InMemoryKeyring,
        path: std::path::PathBuf,
    }

    async fn device(name: &str) -> Device {
        let (pool, path) = migrated_pool(name).await;
        Device {
            pool,
            store: InMemoryKeyring::new(),
            path,
        }
    }

    #[tokio::test]
    async fn invite_round_trips_and_rejects_foreign_links() {
        let d = device("rg_p2p_invite.db").await;
        let group = ReadingGroupService::new(d.pool.clone())
            .create_group("Book Club")
            .await
            .expect("group");

        let invite = invite_create(
            &d.pool,
            &d.store,
            &group.id,
            vec!["wss://relay.example".into()],
        )
        .await
        .expect("invite");
        assert!(invite.link.starts_with("mylore://group-invite#"));
        assert_eq!(invite.relays, vec!["wss://relay.example".to_string()]);
        assert!(!invite.key_id.is_empty());

        // The link parses back to the same group id + key.
        let (parsed_id, _key) = imp::invite_link_round_trip_for_test(&invite.link).unwrap();
        assert_eq!(parsed_id, group.id);

        // A foreign link is rejected.
        assert!(invite_accept(&d.pool, &d.store, "https://example.com")
            .await
            .is_err());
        let broken = format!("mylore://group-invite#{}", "!!!not-base64!!!");
        assert!(invite_accept(&d.pool, &d.store, &broken).await.is_err());

        cleanup_files(&d.path);
    }

    #[tokio::test]
    async fn two_devices_converge_on_shared_notes() {
        let a = device("rg_p2p_peer_a.db").await;
        let b = device("rg_p2p_peer_b.db").await;

        // Peer A creates the group + invite; peer B joins (imports the key).
        let group = ReadingGroupService::new(a.pool.clone())
            .create_group("Book Club")
            .await
            .expect("group");
        let invite = invite_create(&a.pool, &a.store, &group.id, vec![])
            .await
            .expect("invite");
        let joined = invite_accept(&b.pool, &b.store, &invite.link)
            .await
            .expect("accept");
        assert_eq!(joined, group.id);

        // A edits a note and produces an encrypted update.
        let from_a = note_edit(&a.pool, &a.store, &group.id, "anilist:42", "n1", "First!")
            .await
            .expect("edit");
        assert_eq!(from_a.notes, vec![entry("n1", "First!")]);
        // The envelope must not leak the plaintext.
        assert!(!from_a.update.contains("First!"));

        // B merges A's update (B has no state vector yet → gets everything).
        let at_b = note_sync(
            &b.pool,
            &b.store,
            &group.id,
            "anilist:42",
            None,
            Some(from_a.update),
        )
        .await
        .expect("sync");
        assert_eq!(at_b.notes, vec![entry("n1", "First!")]);

        // B adds a second note; A merges it.
        let from_b = note_edit(&b.pool, &b.store, &group.id, "anilist:42", "n2", "Second!")
            .await
            .expect("edit b");
        let at_a = note_sync(
            &a.pool,
            &a.store,
            &group.id,
            "anilist:42",
            None,
            Some(from_b.update),
        )
        .await
        .expect("sync a");
        assert_eq!(
            at_a.notes,
            vec![entry("n1", "First!"), entry("n2", "Second!")]
        );

        // Both devices now agree.
        let a_notes = note_state(&a.pool, &group.id, "anilist:42").await.unwrap();
        let b_notes = note_state(&b.pool, &group.id, "anilist:42").await.unwrap();
        assert_eq!(a_notes, b_notes);

        cleanup_files(&a.path);
        cleanup_files(&b.path);
    }

    #[tokio::test]
    async fn state_vector_handshake_sends_only_missing_updates() {
        let a = device("rg_p2p_sv_a.db").await;
        let b = device("rg_p2p_sv_b.db").await;
        let group = ReadingGroupService::new(a.pool.clone())
            .create_group("G")
            .await
            .expect("group");
        let invite = invite_create(&a.pool, &a.store, &group.id, vec![])
            .await
            .unwrap();
        invite_accept(&b.pool, &b.store, &invite.link)
            .await
            .unwrap();

        // A writes a note; B syncs and ends up identical.
        let from_a = note_edit(&a.pool, &a.store, &group.id, "w", "n1", "hello")
            .await
            .unwrap();
        let at_b = note_sync(&b.pool, &b.store, &group.id, "w", None, Some(from_a.update))
            .await
            .unwrap();
        let b_sv = at_b.state_vector.clone();

        // A asks for B's missing ops, knowing B's state vector: nothing new.
        let quiet = note_sync(&a.pool, &a.store, &group.id, "w", Some(b_sv), None)
            .await
            .unwrap();
        assert_eq!(quiet.notes, vec![entry("n1", "hello")]);
        assert!(!quiet.compacted);

        cleanup_files(&a.path);
        cleanup_files(&b.path);
    }

    #[tokio::test]
    async fn compaction_triggers_at_the_ops_threshold() {
        let d = device("rg_p2p_compact.db").await;
        let group = ReadingGroupService::new(d.pool.clone())
            .create_group("G")
            .await
            .expect("group");

        let mut compacted_on = None;
        for i in 1..=101 {
            let view = note_edit(&d.pool, &d.store, &group.id, "w", "n1", &format!("v{i}"))
                .await
                .expect("edit");
            if view.compacted {
                compacted_on = Some(i);
                break;
            }
        }
        assert_eq!(
            compacted_on,
            Some(100),
            "compaction fires at the op threshold"
        );
        // The doc survives compaction.
        let notes = note_state(&d.pool, &group.id, "w").await.unwrap();
        assert_eq!(notes, vec![entry("n1", "v100")]);

        cleanup_files(&d.path);
    }

    #[test]
    fn envelope_open_rejects_a_wrong_key_or_aad() {
        let key = [7u8; 32];
        let other = [9u8; 32];
        let plaintext = b"secret note body";

        let envelope = imp::seal_for_test(&key, b"g|0", plaintext).unwrap();
        // Correct key + AAD → plaintext back.
        assert_eq!(
            imp::open_for_test(&key, b"g|0", &envelope).unwrap(),
            plaintext
        );
        // Wrong AAD (e.g. after an epoch bump) fails closed…
        assert!(imp::open_for_test(&key, b"g|1", &envelope).is_err());
        // …and so does a wrong key.
        assert!(imp::open_for_test(&other, b"g|0", &envelope).is_err());
    }

    fn entry(note_id: &str, body: &str) -> GroupNoteEntry {
        GroupNoteEntry {
            note_id: note_id.to_string(),
            body: body.to_string(),
        }
    }
}

#[cfg(all(test, not(feature = "p2p")))]
mod stub_tests {
    use crate::infrastructure::keyring::InMemoryKeyring;
    use crate::infrastructure::test_support::{cleanup_files, migrated_pool};

    #[tokio::test]
    async fn default_build_reports_p2p_is_unsupported() {
        let (pool, path) = migrated_pool("rg_p2p_stub.db").await;
        let store = InMemoryKeyring::new();
        let err = super::note_sync(&pool, &store, "g-1", "w", None, None)
            .await
            .expect_err("no p2p");
        assert!(
            err.to_string().contains("p2p"),
            "the error names the missing feature: {err}"
        );
        cleanup_files(&path);
    }
}
