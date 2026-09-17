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

/// The reserved topic that carries group state (roster + shelves) rather than a
/// work's notes. A work key is `provider:value` or `h:<hex>`, so it can never
/// collide with one (MISSION-118).
pub const STATE_TOPIC: &str = "~state";

/// One member as the state document knows them.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct GroupStateMember {
    pub member_id: String,
    pub display_name: String,
}

/// One shelf entry as the state document knows it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct GroupStateShelfEntry {
    pub member_id: String,
    pub work_key: String,
    pub title: String,
    pub content_type: String,
    pub status: String,
    pub progress: i64,
}

/// What the reserved state document currently holds: who has announced
/// themselves into the group, and where each of them is in each work.
///
/// Membership *removal* is deliberately not expressed here. Every accepted
/// invite makes the joiner the owner of its own replica, so two devices would
/// publish conflicting lists; a removal is enforced by rotating the group key
/// instead (see `rotate_group_key`), and the entries a removed member already
/// published stay in the copies they were shared with.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct GroupStateView {
    pub members: Vec<GroupStateMember>,
    pub shelf: Vec<GroupStateShelfEntry>,
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

    use crate::application::reading_group_service::ReadingGroupService;
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
    /// Envelope plaintext framing, so a payload can say which work it carries
    /// *inside* the sealed bytes instead of in a relay-readable tag (MISSION-118).
    const ENVELOPE_MAGIC: &[u8; 4] = b"MLG1";
    /// Field separator inside a state-document composite key. Member ids and work
    /// keys are UTF-8 text that never contains NUL.
    const KEY_SEP: char = '\u{0}';

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

    /// Replace the group key with a fresh one (forward secrecy). Everything
    /// sealed afterwards is unreadable to anyone still holding the old key, which
    /// is the point: a removed member cannot follow the group any further.
    fn rotate_key(store: &dyn SecretStore, group_id: &str) -> Result<[u8; 32], AppError> {
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

    /// Seal a framed envelope: `magic || topic || NUL || update`.
    ///
    /// The topic (a work key, or `~state`) travels **inside** the ciphertext, so
    /// the relay-facing event needs no tag naming the work (MISSION-118).
    fn seal_update(
        key: &[u8; 32],
        aad: &[u8],
        work_key: &str,
        update: &[u8],
    ) -> Result<Vec<u8>, AppError> {
        let mut plain =
            Vec::with_capacity(ENVELOPE_MAGIC.len() + work_key.len() + 1 + update.len());
        plain.extend_from_slice(ENVELOPE_MAGIC);
        plain.extend_from_slice(work_key.as_bytes());
        plain.push(0);
        plain.extend_from_slice(update);
        seal(key, aad, &plain)
    }

    /// Decrypt one sealed envelope and split out the work it belongs to and its
    /// still-unapplied `yrs` update.
    fn open_envelope(
        key: &[u8; 32],
        aad: &[u8],
        envelope: &[u8],
    ) -> Result<(String, Vec<u8>), AppError> {
        let plaintext = open(key, aad, envelope)?;
        let body = plaintext
            .strip_prefix(ENVELOPE_MAGIC.as_slice())
            .ok_or_else(|| AppError::validation("unsupported group envelope version"))?;
        let split = body
            .iter()
            .position(|byte| *byte == 0)
            .ok_or_else(|| AppError::validation("group envelope is malformed"))?;
        let work_key = std::str::from_utf8(&body[..split])
            .map_err(|_| AppError::validation("group envelope topic is not UTF-8"))?;
        if work_key.is_empty() {
            return Err(AppError::validation("group envelope has no topic"));
        }
        Ok((work_key.to_string(), body[split + 1..].to_vec()))
    }

    fn apply_update_bytes(doc: &Doc, update: &[u8]) -> Result<(), AppError> {
        let update = Update::decode_v1(update)
            .map_err(|e| AppError::validation(format!("invalid group update: {e}")))?;
        doc.transact_mut()
            .apply_update(update)
            .map_err(|e| AppError::validation(format!("group update rejected: {e}")))?;
        Ok(())
    }

    /// Decrypt + apply one envelope into `doc`, refusing a payload that belongs
    /// to a different work than the caller asked for.
    fn merge_for_topic(
        doc: &Doc,
        key: &[u8; 32],
        aad: &[u8],
        expected_topic: &str,
        envelope: &[u8],
    ) -> Result<(), AppError> {
        let (topic, update) = open_envelope(key, aad, envelope)?;
        if topic != expected_topic {
            return Err(AppError::validation(
                "group envelope belongs to a different work",
            ));
        }
        apply_update_bytes(doc, &update)
    }

    /// Persist a document, applying the compaction policy. Returns whether the
    /// document was compacted by this call.
    async fn persist_doc(
        pool: &SqlitePool,
        group_id: &str,
        work_key: &str,
        doc: &Doc,
        existing: Option<&DocRecord>,
        applied_ops: i64,
    ) -> Result<bool, AppError> {
        let now = chrono::Utc::now().to_rfc3339();
        let pending_ops = existing.map(|d| d.pending_ops).unwrap_or(0) + applied_ops;
        let compacted_at = existing.and_then(|d| d.compacted_at.clone());
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
                state: encode_full_state(doc),
                pending_ops,
                compacted_at,
                updated_at: now,
            },
        )
        .await?;
        Ok(compacted)
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
            // The replica belongs to *this* device's member identity, so the
            // local owner check (settings/keys are owner-only) recognises us.
            let member_id = ReadingGroupService::new(pool.clone())
                .prefs()
                .await?
                .member_id;
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
            merge_for_topic(&doc, &key, &aad, work_key, &unb64(&update_b64)?)?;
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
        let envelope = seal_update(&key, &aad, work_key, &update_plain)?;

        // 3. Persist, compacting on the size/time policy.
        let compacted = persist_doc(
            pool,
            group_id,
            work_key,
            &doc,
            existing.as_ref(),
            i64::from(applied),
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

    /// The topic a sealed envelope belongs to, decrypted without touching any
    /// document — so the caller can dedup an arriving event before applying it.
    pub async fn topic_of(
        pool: &SqlitePool,
        store: &dyn SecretStore,
        group_id: &str,
        envelope: &[u8],
    ) -> Result<String, AppError> {
        let group = repo::get_group(pool, group_id)
            .await?
            .ok_or_else(|| AppError::validation(format!("unknown group: {group_id}")))?;
        let key = load_key(store, group_id)?.ok_or_else(|| {
            AppError::validation("no group key — create or accept an invite first")
        })?;
        let (topic, _) = open_envelope(&key, &aad(group_id, group.epoch), envelope)?;
        Ok(topic)
    }

    /// Apply one incoming envelope to whichever document it belongs to (a work's
    /// notes, or the reserved state document, which is then projected into the
    /// group tables). Returns the topic it carried.
    pub async fn apply_envelope(
        pool: &SqlitePool,
        store: &dyn SecretStore,
        group_id: &str,
        envelope: &[u8],
    ) -> Result<String, AppError> {
        let group = repo::get_group(pool, group_id)
            .await?
            .ok_or_else(|| AppError::validation(format!("unknown group: {group_id}")))?;
        let key = load_key(store, group_id)?.ok_or_else(|| {
            AppError::validation("no group key — create or accept an invite first")
        })?;
        let aad = aad(group_id, group.epoch);
        let (topic, update) = open_envelope(&key, &aad, envelope)?;

        let existing = repo::get_doc(pool, group_id, &topic).await?;
        let doc = open_doc(existing.as_ref().map(|d| d.state.as_slice()).unwrap_or(&[]))?;
        apply_update_bytes(&doc, &update)?;
        persist_doc(pool, group_id, &topic, &doc, existing.as_ref(), 1).await?;

        if topic == STATE_TOPIC {
            project_state(pool, group_id).await?;
        }
        Ok(topic)
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

        let update_plain = encode_full_state(&doc);
        let envelope = seal_update(&key, &aad, work_key, &update_plain)?;
        let own_state_vector = doc.transact().state_vector().encode_v1();
        let compacted = persist_doc(pool, group_id, work_key, &doc, existing.as_ref(), 1).await?;

        Ok(GroupNoteSyncView {
            group_id: group_id.to_string(),
            work_key: work_key.to_string(),
            state_vector: b64(&own_state_vector),
            update: b64(&envelope),
            notes: entries(&doc),
            compacted,
        })
    }

    // ------------------------------------------------------------------ state

    const STATE_MEMBERS: &str = "members";
    const STATE_SHELF: &str = "shelf";

    #[derive(serde::Serialize, serde::Deserialize)]
    struct ShelfValue {
        title: String,
        content_type: String,
        status: String,
        progress: i64,
    }

    fn composite(member_id: &str, work_key: &str) -> String {
        format!("{member_id}{KEY_SEP}{work_key}")
    }

    fn set_state_member(doc: &Doc, member_id: &str, display_name: &str) {
        let map = doc.get_or_insert_map(STATE_MEMBERS);
        map.insert(&mut doc.transact_mut(), member_id, display_name);
    }

    fn set_state_shelf(doc: &Doc, entry: &GroupStateShelfEntry) -> Result<(), AppError> {
        let map = doc.get_or_insert_map(STATE_SHELF);
        let value = serde_json::to_string(&ShelfValue {
            title: entry.title.clone(),
            content_type: entry.content_type.clone(),
            status: entry.status.clone(),
            progress: entry.progress,
        })?;
        map.insert(
            &mut doc.transact_mut(),
            composite(&entry.member_id, &entry.work_key).as_str(),
            value,
        );
        Ok(())
    }

    fn state_view(doc: &Doc) -> GroupStateView {
        let member_map = doc.get_or_insert_map(STATE_MEMBERS);
        let shelf_map = doc.get_or_insert_map(STATE_SHELF);
        let txn = doc.transact();

        let mut members: Vec<GroupStateMember> = member_map
            .iter(&txn)
            .map(|(member_id, name)| GroupStateMember {
                member_id: member_id.to_string(),
                display_name: name.to_string(&txn),
            })
            .collect();
        members.sort_by(|a, b| a.member_id.cmp(&b.member_id));

        let mut shelf: Vec<GroupStateShelfEntry> = shelf_map
            .iter(&txn)
            .filter_map(|(key, value)| {
                let (member_id, work_key) = key.split_once(KEY_SEP)?;
                let parsed: ShelfValue = serde_json::from_str(&value.to_string(&txn)).ok()?;
                Some(GroupStateShelfEntry {
                    member_id: member_id.to_string(),
                    work_key: work_key.to_string(),
                    title: parsed.title,
                    content_type: parsed.content_type,
                    status: parsed.status,
                    progress: parsed.progress,
                })
            })
            .collect();
        shelf.sort_by(|a, b| {
            a.member_id
                .cmp(&b.member_id)
                .then_with(|| a.work_key.cmp(&b.work_key))
        });

        GroupStateView { members, shelf }
    }

    /// Publish this device's own rows — its member entry and its shelf — and
    /// return the sealed envelope to broadcast, or `None` when the document would
    /// not change.
    ///
    /// Each device writes only its own keys, so the merge stays conflict-free:
    /// this is how the other devices learn who is in the group and where they
    /// are, which is what fills in the shelves matrix.
    pub async fn state_announce(
        pool: &SqlitePool,
        store: &dyn SecretStore,
        group_id: &str,
    ) -> Result<Option<String>, AppError> {
        let group = repo::get_group(pool, group_id)
            .await?
            .ok_or_else(|| AppError::validation(format!("unknown group: {group_id}")))?;
        // Without a key nothing can leave the device, so there is nothing to say.
        let Some(key) = load_key(store, group_id)? else {
            return Ok(None);
        };
        let aad = aad(group_id, group.epoch);

        let me = ReadingGroupService::new(pool.clone()).prefs().await?;
        let my_name = if me.display_name.trim().is_empty() {
            "Me".to_string()
        } else {
            me.display_name.trim().to_string()
        };

        let existing = repo::get_doc(pool, group_id, STATE_TOPIC).await?;
        let doc = open_doc(existing.as_ref().map(|d| d.state.as_slice()).unwrap_or(&[]))?;
        let before = state_view(&doc);

        set_state_member(&doc, &me.member_id, &my_name);
        for row in repo::list_shelf(pool, group_id, Some(&me.member_id)).await? {
            set_state_shelf(
                &doc,
                &GroupStateShelfEntry {
                    member_id: row.member_id,
                    work_key: row.work_key,
                    title: row.title,
                    content_type: row.content_type,
                    status: row.status,
                    progress: row.progress,
                },
            )?;
        }

        if state_view(&doc) == before {
            return Ok(None);
        }

        let update_plain = encode_full_state(&doc);
        let envelope = seal_update(&key, &aad, STATE_TOPIC, &update_plain)?;
        persist_doc(pool, group_id, STATE_TOPIC, &doc, existing.as_ref(), 1).await?;
        Ok(Some(b64(&envelope)))
    }

    /// Fold the state document into the group tables: an announced member who is
    /// not known yet becomes a `group_member` row, and their shelf entries become
    /// `group_shelf` rows. Returns the view it applied.
    ///
    /// It only adds or refreshes — never deletes. Membership revocation is the
    /// key rotation's job; a member who is gone stays in the history they were
    /// part of.
    pub async fn project_state(
        pool: &SqlitePool,
        group_id: &str,
    ) -> Result<GroupStateView, AppError> {
        let existing = repo::get_doc(pool, group_id, STATE_TOPIC).await?;
        let doc = open_doc(existing.as_ref().map(|d| d.state.as_slice()).unwrap_or(&[]))?;
        let view = state_view(&doc);

        let me = ReadingGroupService::new(pool.clone())
            .prefs()
            .await?
            .member_id;
        let now = chrono::Utc::now().to_rfc3339();

        for member in &view.members {
            match repo::get_member(pool, group_id, &member.member_id).await? {
                // An announcement never rewrites the local owner's own row.
                Some(row) if row.role == "owner" => {}
                Some(_) | None => {
                    repo::add_member(
                        pool,
                        &repo::MemberRecord {
                            group_id: group_id.to_string(),
                            member_id: member.member_id.clone(),
                            display_name: member.display_name.clone(),
                            role: "member".to_string(),
                            joined_at: now.clone(),
                        },
                    )
                    .await?
                }
            }
        }

        for entry in &view.shelf {
            // My own shelf is written by my own edits; the document carries the
            // other members' rows (single-writer per member).
            if entry.member_id == me {
                continue;
            }
            repo::upsert_shelf(
                pool,
                &repo::ShelfRecord {
                    group_id: group_id.to_string(),
                    member_id: entry.member_id.clone(),
                    work_key: entry.work_key.clone(),
                    title: entry.title.clone(),
                    content_type: entry.content_type.clone(),
                    status: entry.status.clone(),
                    progress: entry.progress,
                    updated_at: now.clone(),
                },
            )
            .await?;
        }

        Ok(view)
    }

    // --------------------------------------------------------------- rotation

    async fn require_owner(pool: &SqlitePool, group_id: &str) -> Result<(), AppError> {
        let service = ReadingGroupService::new(pool.clone());
        let group = service.view_group(group_id).await?;
        let prefs = service.prefs().await?;
        if group.owner_id != prefs.member_id {
            return Err(AppError::validation(
                "only the group owner can rotate the group key",
            ));
        }
        Ok(())
    }

    /// Rotate a group's key and bump its epoch, so anything sealed from now on is
    /// unreadable to whoever held the previous key. Owner-only: re-keying is what
    /// a removal *means*, and a non-owner doing it would lock the group out.
    ///
    /// Queued envelopes are dropped — they were sealed under the old key and the
    /// first sync after re-keying sends a full state diff anyway.
    pub async fn rotate_group_key(
        pool: &SqlitePool,
        store: &dyn SecretStore,
        group_id: &str,
    ) -> Result<GroupKeyStatus, AppError> {
        require_owner(pool, group_id).await?;
        let key = rotate_key(store, group_id)?;
        repo::bump_epoch(pool, group_id, &chrono::Utc::now().to_rfc3339()).await?;
        repo::clear_outbox(pool, group_id).await?;
        Ok(GroupKeyStatus {
            has_key: true,
            key_id: Some(fingerprint(&key)),
        })
    }

    /// Called after the owner removes a member: rotate when a key exists.
    /// Returns whether rotation happened (a group with no key has nothing to
    /// revoke).
    pub async fn rotate_after_removal(
        pool: &SqlitePool,
        store: &dyn SecretStore,
        group_id: &str,
    ) -> Result<bool, AppError> {
        if load_key(store, group_id)?.is_none() {
            return Ok(false);
        }
        rotate_group_key(pool, store, group_id).await?;
        Ok(true)
    }

    /// The group state the membership table implies, for tests and inspection.
    pub async fn state_view_for(
        pool: &SqlitePool,
        group_id: &str,
    ) -> Result<GroupStateView, AppError> {
        let existing = repo::get_doc(pool, group_id, STATE_TOPIC).await?;
        let doc = open_doc(existing.as_ref().map(|d| d.state.as_slice()).unwrap_or(&[]))?;
        Ok(state_view(&doc))
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

    /// Exposed for tests: the compaction policy against a given clock reading.
    #[cfg(test)]
    pub fn should_compact_for_test(pending_ops: i64, compacted_at: Option<&str>) -> bool {
        should_compact(pending_ops, compacted_at)
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

    pub async fn apply_envelope(
        _pool: &SqlitePool,
        _store: &dyn SecretStore,
        _group_id: &str,
        _envelope: &[u8],
    ) -> Result<String, AppError> {
        unsupported()
    }

    pub async fn topic_of(
        _pool: &SqlitePool,
        _store: &dyn SecretStore,
        _group_id: &str,
        _envelope: &[u8],
    ) -> Result<String, AppError> {
        unsupported()
    }

    pub async fn state_announce(
        _pool: &SqlitePool,
        _store: &dyn SecretStore,
        _group_id: &str,
    ) -> Result<Option<String>, AppError> {
        unsupported()
    }

    pub async fn project_state(
        _pool: &SqlitePool,
        _group_id: &str,
    ) -> Result<GroupStateView, AppError> {
        unsupported()
    }

    pub async fn state_view_for(
        _pool: &SqlitePool,
        _group_id: &str,
    ) -> Result<GroupStateView, AppError> {
        unsupported()
    }

    pub async fn rotate_group_key(
        _pool: &SqlitePool,
        _store: &dyn SecretStore,
        _group_id: &str,
    ) -> Result<GroupKeyStatus, AppError> {
        unsupported()
    }

    /// A build without the feature holds no group key, so a removal has nothing
    /// to revoke. This is the one entry point that succeeds rather than rejecting:
    /// member removal must keep working in a local-only build.
    pub async fn rotate_after_removal(
        _pool: &SqlitePool,
        _store: &dyn SecretStore,
        _group_id: &str,
    ) -> Result<bool, AppError> {
        Ok(false)
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

pub use imp::{
    apply_envelope, invite_accept, invite_create, key_status, note_edit, note_state, note_sync,
    project_state, rotate_after_removal, rotate_group_key, state_announce, state_view_for,
    topic_of,
};

#[cfg(all(test, feature = "p2p"))]
mod tests {
    use super::*;
    use crate::application::reading_group_service::ReadingGroupService;
    use crate::infrastructure::keyring::InMemoryKeyring;
    use crate::infrastructure::repositories::reading_group as repo;
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

    // ------------------------------------------- MISSION-118 · envelope topic

    /// The work a payload is about travels inside the ciphertext, so a relay can
    /// never read it, and an envelope cannot be replayed into another work's
    /// document.
    #[tokio::test]
    async fn the_work_key_rides_inside_the_sealed_envelope() {
        let a = device("rg118_topic_a.db").await;
        let b = device("rg118_topic_b.db").await;
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

        let from_a = note_edit(&a.pool, &a.store, &group.id, "anilist:42", "n1", "spoiler")
            .await
            .expect("edit");
        // Neither the work key nor the body may be readable from the envelope.
        assert!(!from_a.update.contains("anilist"));
        assert!(!from_a.update.contains("spoiler"));

        // The topic is recoverable with the key, and it is the work we wrote to.
        let envelope =
            crate::application::reading_group_transport::envelope_bytes(&from_a.update).unwrap();
        let topic = topic_of(&b.pool, &b.store, &group.id, &envelope)
            .await
            .expect("B can open what A sealed");
        assert_eq!(topic, "anilist:42");

        // An envelope for one work is refused by another work's sync.
        let refused = note_sync(
            &b.pool,
            &b.store,
            &group.id,
            "h:other-work",
            None,
            Some(from_a.update.clone()),
        )
        .await;
        assert!(
            refused.is_err(),
            "an envelope must not be merged into a work it does not belong to"
        );

        cleanup_files(&a.path);
        cleanup_files(&b.path);
    }

    #[tokio::test]
    async fn a_truncated_envelope_is_refused_without_touching_the_document() {
        let d = device("rg118_truncated.db").await;
        let group = ReadingGroupService::new(d.pool.clone())
            .create_group("G")
            .await
            .expect("group");
        let from_d = note_edit(&d.pool, &d.store, &group.id, "w", "n1", "kept")
            .await
            .unwrap();
        let envelope =
            crate::application::reading_group_transport::envelope_bytes(&from_d.update).unwrap();

        assert!(topic_of(&d.pool, &d.store, &group.id, &envelope[..10])
            .await
            .is_err());
        assert!(
            apply_envelope(&d.pool, &d.store, &group.id, &envelope[..10])
                .await
                .is_err()
        );
        // The document still holds exactly the note that was written.
        assert_eq!(
            note_state(&d.pool, &group.id, "w").await.unwrap(),
            vec![entry("n1", "kept")]
        );

        cleanup_files(&d.path);
    }

    /// An envelope sealed under an epoch the group has moved past is not ours any
    /// more: this is what makes a re-key take effect (clock skew included — the
    /// AAD is the epoch, not a timestamp).
    #[tokio::test]
    async fn an_envelope_from_a_stale_epoch_is_refused_after_a_rotation() {
        let d = device("rg118_stale_epoch.db").await;
        let group = ReadingGroupService::new(d.pool.clone())
            .create_group("G")
            .await
            .expect("group");
        // A key must exist before a rotation has anything to replace.
        invite_create(&d.pool, &d.store, &group.id, vec![])
            .await
            .unwrap();

        let before = note_edit(&d.pool, &d.store, &group.id, "w", "n1", "sealed at epoch 0")
            .await
            .unwrap();
        let stale =
            crate::application::reading_group_transport::envelope_bytes(&before.update).unwrap();
        assert!(topic_of(&d.pool, &d.store, &group.id, &stale).await.is_ok());

        let rotated = rotate_group_key(&d.pool, &d.store, &group.id)
            .await
            .unwrap();
        assert!(rotated.has_key);

        // The old envelope no longer opens with the group's current key.
        assert!(topic_of(&d.pool, &d.store, &group.id, &stale)
            .await
            .is_err());
        assert!(apply_envelope(&d.pool, &d.store, &group.id, &stale)
            .await
            .is_err());

        cleanup_files(&d.path);
    }

    // --------------------------------------------- MISSION-118 · key rotation

    #[tokio::test]
    async fn rotating_the_key_changes_the_fingerprint_and_bumps_the_epoch() {
        let d = device("rg118_rotate.db").await;
        let group = ReadingGroupService::new(d.pool.clone())
            .create_group("G")
            .await
            .expect("group");
        let invite = invite_create(&d.pool, &d.store, &group.id, vec![])
            .await
            .unwrap();

        let rotated = rotate_group_key(&d.pool, &d.store, &group.id)
            .await
            .unwrap();
        assert_ne!(rotated.key_id, Some(invite.key_id.clone()));
        assert_eq!(
            key_status(&d.store, &group.id).await.unwrap().key_id,
            rotated.key_id
        );

        let after = repo::get_group(&d.pool, &group.id).await.unwrap().unwrap();
        assert_eq!(after.epoch, 1, "an epoch bump invalidates the old AAD");

        cleanup_files(&d.path);
    }

    #[tokio::test]
    async fn only_the_owner_may_rotate_the_group_key() {
        let d = device("rg118_rotate_nonowner.db").await;
        let group = ReadingGroupService::new(d.pool.clone())
            .create_group("G")
            .await
            .expect("group");
        invite_create(&d.pool, &d.store, &group.id, vec![])
            .await
            .unwrap();

        // A replica that does not own this group must not re-key it for everyone.
        sqlx::query("UPDATE reading_group SET owner_id = 'm-somebody-else' WHERE id = ?")
            .bind(&group.id)
            .execute(&d.pool)
            .await
            .unwrap();

        let error = rotate_group_key(&d.pool, &d.store, &group.id)
            .await
            .expect_err("a non-owner must be refused");
        assert!(error.to_string().contains("owner"), "{error}");

        cleanup_files(&d.path);
    }

    #[tokio::test]
    async fn removal_only_rotates_when_a_key_exists() {
        let d = device("rg118_removal.db").await;
        let group = ReadingGroupService::new(d.pool.clone())
            .create_group("G")
            .await
            .expect("group");

        // No key yet: nothing to revoke, and the removal must still succeed.
        assert!(!rotate_after_removal(&d.pool, &d.store, &group.id)
            .await
            .unwrap());

        let invite = invite_create(&d.pool, &d.store, &group.id, vec![])
            .await
            .unwrap();
        assert!(rotate_after_removal(&d.pool, &d.store, &group.id)
            .await
            .unwrap());
        let after = key_status(&d.store, &group.id).await.unwrap();
        assert_ne!(after.key_id, Some(invite.key_id.clone()));

        cleanup_files(&d.path);
    }

    // ------------------------------------------- MISSION-118 · state over wire

    #[tokio::test]
    async fn the_state_document_carries_members_and_shelves_to_a_peer() {
        let a = device("rg118_state_a.db").await;
        let b = device("rg118_state_b.db").await;
        let service_a = ReadingGroupService::new(a.pool.clone());
        let group = service_a.create_group("G").await.expect("group");
        let invite = invite_create(&a.pool, &a.store, &group.id, vec![])
            .await
            .unwrap();
        invite_accept(&b.pool, &b.store, &invite.link)
            .await
            .unwrap();

        // A shelves a work; every accepted invite makes B the owner of its own
        // replica, so A stays the owner of *its* copy — "A" here is A's device.
        let me_a = service_a.prefs().await.unwrap().member_id;
        service_a.set_prefs(true, "A").await.unwrap();
        service_a
            .set_shelf_entry(
                &group.id,
                &me_a,
                "anilist:42",
                "Berserk",
                "manga",
                "in_progress",
                12,
            )
            .await
            .expect("shelf");

        // A announces: the announcement is a real envelope.
        let sealed = state_announce(&a.pool, &a.store, &group.id)
            .await
            .expect("announce")
            .expect("the shelf change changed the document");
        let envelope =
            crate::application::reading_group_transport::envelope_bytes(&sealed).unwrap();
        assert_eq!(
            topic_of(&a.pool, &a.store, &group.id, &envelope)
                .await
                .unwrap(),
            STATE_TOPIC
        );

        // B merges it and the group tables learn who A is and where A is.
        apply_envelope(&b.pool, &b.store, &group.id, &envelope)
            .await
            .expect("apply");

        let view = ReadingGroupService::new(b.pool.clone())
            .view_group(&group.id)
            .await
            .unwrap();
        let a_member = view
            .members
            .iter()
            .find(|member| member.member_id == me_a)
            .expect("A is announced to B");
        assert_eq!(a_member.display_name, "A");

        let shelf = repo::list_shelf(&b.pool, &group.id, None).await.unwrap();
        let row = shelf
            .iter()
            .find(|row| row.member_id == me_a && row.work_key == "anilist:42")
            .expect("A's shelf row reached B");
        assert_eq!(row.progress, 12);
        assert_eq!(row.title, "Berserk");
        assert_eq!(row.status, "in_progress");

        cleanup_files(&a.path);
        cleanup_files(&b.path);
    }

    #[tokio::test]
    async fn announcing_twice_publishes_once() {
        let d = device("rg118_announce_once.db").await;
        let group = ReadingGroupService::new(d.pool.clone())
            .create_group("G")
            .await
            .expect("group");
        invite_create(&d.pool, &d.store, &group.id, vec![])
            .await
            .unwrap();

        assert!(state_announce(&d.pool, &d.store, &group.id)
            .await
            .unwrap()
            .is_some());
        assert!(
            state_announce(&d.pool, &d.store, &group.id)
                .await
                .unwrap()
                .is_none(),
            "an unchanged announcement must not add a second envelope"
        );

        cleanup_files(&d.path);
    }

    #[tokio::test]
    async fn a_group_without_a_key_announces_nothing() {
        let d = device("rg118_announce_no_key.db").await;
        let group = ReadingGroupService::new(d.pool.clone())
            .create_group("G")
            .await
            .expect("group");

        assert!(state_announce(&d.pool, &d.store, &group.id)
            .await
            .unwrap()
            .is_none());

        cleanup_files(&d.path);
    }

    // ------------------------------------------------ MISSION-118 · chaos

    /// A skewed clock must never make the compaction policy panic or compact
    /// early: a future or malformed stamp simply does not trigger it.
    #[test]
    fn the_compaction_policy_survives_a_skewed_clock() {
        assert!(!imp::should_compact_for_test(
            1,
            Some("2999-01-01T00:00:00Z")
        ));
        assert!(!imp::should_compact_for_test(1, Some("not a timestamp")));
        assert!(!imp::should_compact_for_test(1, None));
        assert!(
            imp::should_compact_for_test(100, None),
            "the op cap still applies"
        );
    }

    #[tokio::test]
    async fn a_foreign_envelope_never_disturbs_the_document() {
        let a = device("rg118_foreign_a.db").await;
        let b = device("rg118_foreign_b.db").await;
        let group_a = ReadingGroupService::new(a.pool.clone())
            .create_group("G")
            .await
            .expect("group");
        let group_b = ReadingGroupService::new(b.pool.clone())
            .create_group("Other")
            .await
            .expect("group");

        note_edit(&a.pool, &a.store, &group_a.id, "w", "n1", "A's note")
            .await
            .unwrap();
        let mine = note_edit(&b.pool, &b.store, &group_b.id, "w", "n1", "B's note")
            .await
            .unwrap();
        let foreign =
            crate::application::reading_group_transport::envelope_bytes(&mine.update).unwrap();

        // B's envelope, offered to A's group: refused, and A's document is intact.
        assert!(apply_envelope(&a.pool, &a.store, &group_a.id, &foreign)
            .await
            .is_err());
        assert_eq!(
            note_state(&a.pool, &group_a.id, "w").await.unwrap(),
            vec![entry("n1", "A's note")]
        );

        cleanup_files(&a.path);
        cleanup_files(&b.path);
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
